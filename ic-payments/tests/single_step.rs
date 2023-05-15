use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use candid::{Encode, Nat};
use ic_canister::{register_raw_virtual_responder, register_virtual_responder};
use ic_exports::ic_cdk::api::call::RejectionCode;
use ic_exports::ic_icrc1::endpoints::{TransferArg, TransferError};
use ic_exports::ic_icrc1::Account;
use ic_exports::ic_kit::mock_principals::alice;
use ic_payments::error::{PaymentError, RecoveryDetails, TransferFailReason};
use ic_payments::recovery_list::{RecoveryList, StableRecoveryList};
use ic_payments::{Operation, Transfer};

use crate::common::{
    init_test, minting_account, setup_error, setup_success, simple_transfer, token_principal,
    TestBalances,
};

pub mod common;

#[tokio::test]
async fn transfer_args() {
    let mut terminal = init_test();
    let transfer = Transfer {
        from: Some([3; 32]),
        to: Account {
            owner: alice().into(),
            subaccount: Some([4; 32]),
        },
        fee: 10.into(),
        ..simple_transfer()
    };
    let transfer_clone = transfer.clone();

    register_virtual_responder(
        token_principal(),
        "icrc1_transfer",
        move |(args,): (TransferArg,)| {
            assert_eq!(args.amount, transfer.amount() - 10);
            assert_eq!(args.fee, Some(10.into()));
            assert_eq!(args.from_subaccount, Some([3; 32]));
            assert_eq!(args.to, transfer.to());
            assert_eq!(args.created_at_time, Some(transfer.created_at()));

            Ok::<Nat, TransferError>(1.into())
        },
    );

    let result = terminal.transfer(transfer_clone, 1).await;
    assert_eq!(result, Ok(1.into()));
}

#[tokio::test]
async fn credit_on_success() {
    let mut terminal = init_test();
    setup_success(1);
    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        fee: 10.into(),
        operation: Operation::CreditOnSuccess,
        ..simple_transfer()
    };

    terminal.transfer(transfer, 1).await.unwrap();
    assert_eq!(TestBalances::balance_of(alice()), 990);
}

#[tokio::test]
async fn credit_on_failure() {
    let mut terminal = init_test();
    setup_error();
    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        operation: Operation::CreditOnError,
        ..simple_transfer()
    };

    terminal.transfer(transfer, 1).await.unwrap_err();
    assert_eq!(TestBalances::balance_of(alice()), 1000);
}

#[tokio::test]
async fn none_operation() {
    let mut terminal = init_test();
    setup_error();
    let transfer = Transfer {
        caller: alice(),
        operation: Operation::None,
        ..simple_transfer()
    };

    terminal.transfer(transfer.clone(), 1).await.unwrap_err();
    assert_eq!(TestBalances::balance_of(alice()), 0);

    setup_success(1);
    terminal.transfer(transfer, 1).await.unwrap();
    assert_eq!(TestBalances::balance_of(alice()), 0);
}

#[tokio::test]
async fn retry_with_success() {
    let mut terminal = init_test();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
        if counter.load(Ordering::Relaxed) >= 2 {
            let response: Result<Nat, TransferError> = Ok(1.into());
            let response_bytes = Encode!(&response).unwrap();
            Ok(response_bytes)
        } else {
            Err((RejectionCode::SysTransient, "recoverable".into()))
        }
    });

    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        fee: 10.into(),
        operation: Operation::CreditOnSuccess,
        ..simple_transfer()
    };

    let tx_id = terminal.transfer(transfer, 3).await.unwrap();
    assert_eq!(tx_id, 1);
    assert_eq!(TestBalances::balance_of(alice()), 990);
    assert_eq!(counter_clone.load(Ordering::Relaxed), 2);
    assert_eq!(StableRecoveryList::<0>.take_all().len(), 0);
}

#[tokio::test]
async fn retry_with_failure() {
    let mut terminal = init_test();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
        if counter.load(Ordering::Relaxed) >= 2 {
            let response: Result<Nat, TransferError> =
                Err(TransferError::InsufficientFunds { balance: 0.into() });
            let response_bytes = Encode!(&response).unwrap();
            Ok(response_bytes)
        } else {
            Err((RejectionCode::SysTransient, "recoverable".into()))
        }
    });

    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        fee: 10.into(),
        operation: Operation::CreditOnSuccess,
        ..simple_transfer()
    };

    let err = terminal.transfer(transfer, 3).await.unwrap_err();
    assert_eq!(
        err,
        PaymentError::TransferFailed(TransferFailReason::Rejected(
            TransferError::InsufficientFunds { balance: 0.into() }
        ))
    );
    assert_eq!(TestBalances::balance_of(alice()), 0);
    assert_eq!(counter_clone.load(Ordering::Relaxed), 2);
    assert_eq!(StableRecoveryList::<0>.take_all().len(), 0);
}

#[tokio::test]
async fn retry_with_maybe_failure() {
    let mut terminal = init_test();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
        Err((RejectionCode::SysTransient, "recoverable".into()))
    });

    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        fee: 10.into(),
        operation: Operation::CreditOnSuccess,
        ..simple_transfer()
    };

    let err = terminal.transfer(transfer, 3).await.unwrap_err();
    assert_eq!(err, PaymentError::Recoverable(RecoveryDetails::IcError));
    assert_eq!(TestBalances::balance_of(alice()), 0);
    assert_eq!(counter_clone.load(Ordering::Relaxed), 3);
    assert_eq!(StableRecoveryList::<0>.take_all().len(), 1);
}

#[tokio::test]
async fn recovery_with_success() {
    let mut terminal = init_test();

    register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
        Err((RejectionCode::SysTransient, "recoverable".into()))
    });

    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        fee: 10.into(),
        operation: Operation::CreditOnSuccess,
        ..simple_transfer()
    };

    terminal.transfer(transfer, 3).await.unwrap_err();

    setup_success(1);

    let results = terminal.recover_all().await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].as_ref().unwrap().0, 1);
    assert_eq!(TestBalances::balance_of(alice()), 990);
    assert_eq!(StableRecoveryList::<0>.take_all().len(), 0);
}

#[tokio::test]
async fn recovery_with_failure() {
    let mut terminal = init_test();

    register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
        Err((RejectionCode::SysTransient, "recoverable".into()))
    });

    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        fee: 10.into(),
        operation: Operation::CreditOnSuccess,
        ..simple_transfer()
    };

    terminal.transfer(transfer, 3).await.unwrap_err();

    setup_error();

    let results = terminal.recover_all().await;
    assert_eq!(results.len(), 1);
    assert!(matches!(
        results[0],
        Err(PaymentError::TransferFailed(TransferFailReason::Rejected(
            TransferError::InsufficientFunds { .. }
        )))
    ));
    assert_eq!(TestBalances::balance_of(alice()), 0);
    assert_eq!(StableRecoveryList::<0>.take_all().len(), 0);
}

#[tokio::test]
async fn recovery_with_maybe_failure() {
    let mut terminal = init_test();

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
        Err((RejectionCode::SysTransient, "recoverable".into()))
    });

    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        fee: 10.into(),
        operation: Operation::CreditOnSuccess,
        ..simple_transfer()
    };

    terminal.transfer(transfer, 3).await.unwrap_err();

    let results = terminal.recover_all().await;
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].as_ref().unwrap_err(),
        &PaymentError::Recoverable(RecoveryDetails::IcError)
    );
    assert_eq!(TestBalances::balance_of(alice()), 0);
    assert_eq!(StableRecoveryList::<0>.take_all().len(), 1);
    assert_eq!(counter_clone.load(Ordering::Relaxed), 6);
}

#[tokio::test]
async fn transient_error_on_recovery() {
    let mut terminal = init_test();

    register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
        Err((RejectionCode::SysTransient, "recoverable".into()))
    });

    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        fee: 10.into(),
        operation: Operation::CreditOnSuccess,
        ..simple_transfer()
    };

    terminal.transfer(transfer, 3).await.unwrap_err();

    register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
        Err((
            RejectionCode::CanisterError,
            "token canister is out of cycles".into(),
        ))
    });

    let results = terminal.recover_all().await;
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].as_ref().unwrap_err(),
        &PaymentError::Recoverable(RecoveryDetails::IcError)
    );
    assert_eq!(TestBalances::balance_of(alice()), 0);
    assert_eq!(StableRecoveryList::<0>.list().len(), 1);

    register_virtual_responder(
        token_principal(),
        "icrc1_transfer",
        move |_: (TransferArg,)| Err::<Nat, TransferError>(TransferError::TemporarilyUnavailable),
    );

    let results = terminal.recover_all().await;
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].as_ref().unwrap_err(),
        &PaymentError::Recoverable(RecoveryDetails::IcError)
    );
    assert_eq!(TestBalances::balance_of(alice()), 0);
    assert_eq!(StableRecoveryList::<0>.list().len(), 1);
}

#[tokio::test]
async fn duplicate_transfers() {
    let mut terminal = init_test();
    setup_success(1);
    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        fee: 10.into(),
        operation: Operation::CreditOnSuccess,
        ..simple_transfer()
    };

    terminal.transfer(transfer.clone(), 1).await.unwrap();

    register_virtual_responder(
        token_principal(),
        "icrc1_transfer",
        move |_: (TransferArg,)| {
            Err::<Nat, TransferError>(TransferError::Duplicate {
                duplicate_of: 1.into(),
            })
        },
    );

    terminal.transfer(transfer, 1).await.unwrap_err();
    assert_eq!(TestBalances::balance_of(alice()), 990);
}

#[tokio::test]
async fn bad_fee_rerequest() {
    let config_change_is_called = Arc::new(AtomicBool::new(false));
    let config_change_is_called_clone = config_change_is_called.clone();
    let mut terminal = init_test().on_config_update(move |config| {
        assert_eq!(config.fee, 100);
        assert_eq!(config.minting_account, minting_account());
        config_change_is_called_clone.store(true, Ordering::Relaxed);
    });

    let counter = Arc::new(AtomicUsize::new(0));
    register_virtual_responder(
        token_principal(),
        "icrc1_transfer",
        move |(args,): (TransferArg,)| {
            if counter.fetch_add(1, Ordering::Relaxed) == 0 {
                Err::<Nat, TransferError>(TransferError::BadFee {
                    expected_fee: 100.into(),
                })
            } else {
                assert_eq!(args.fee, Some(100.into()));
                Ok(1.into())
            }
        },
    );

    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        fee: 10.into(),
        operation: Operation::CreditOnSuccess,
        ..simple_transfer()
    };

    terminal.transfer(transfer.clone(), 3).await.unwrap();
    assert_eq!(TestBalances::balance_of(alice()), 900);
    assert_eq!(StableRecoveryList::<0>.list().len(), 0);

    assert_eq!(terminal.fee(), 100);
    assert!(config_change_is_called.load(Ordering::Relaxed));
}
