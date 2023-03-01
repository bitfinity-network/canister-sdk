use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use candid::{Encode, Nat};
use common::{
    init_context, init_test, setup_error, setup_success, simple_transfer, this_principal,
    token_principal, TestBalances,
};
use ic_canister::{register_raw_virtual_responder, register_virtual_responder};
use ic_exports::ic_icrc1::endpoints::{TransferArg, TransferError};
use ic_exports::ic_icrc1::Account;
use ic_exports::ic_kit::mock_principals::alice;
use ic_exports::ic_kit::RejectionCode;
use ic_payments::error::{PaymentError, RecoveryDetails, TransferFailReason};
use ic_payments::recovery_list::ForRecoveryList;
use ic_payments::{Operation, Stage, Transfer, TransferType, UNKNOWN_TX_ID};

mod common;

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
    }
    .double_step();

    terminal.transfer(transfer, 1).await.unwrap();
    assert_eq!(TestBalances::balance_of(alice()), 980);
}

#[tokio::test]
async fn second_stage_rejected() {
    let mut terminal = init_test();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    register_virtual_responder(
        token_principal(),
        "icrc1_transfer",
        move |_: (TransferArg,)| {
            let count = counter.fetch_add(1, Ordering::Relaxed);
            if count == 0 {
                Ok::<Nat, TransferError>(Nat::from(1))
            } else {
                Err::<Nat, TransferError>(TransferError::BadFee {
                    expected_fee: 123.into(),
                })
            }
        },
    );

    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        fee: 10.into(),
        operation: Operation::CreditOnSuccess,
        ..simple_transfer()
    }
    .double_step();

    let err = terminal.transfer(transfer, 1).await.unwrap_err();
    assert_eq!(
        err,
        PaymentError::Recoverable(RecoveryDetails::BadFee(123.into()))
    );
    assert_eq!(counter_clone.load(Ordering::Relaxed), 2);
    assert_eq!(TestBalances::balance_of(alice()), 0);
    assert_eq!(ForRecoveryList::<0>.take_all().len(), 1);
}

#[tokio::test]
async fn credit_on_error() {
    let mut terminal = init_test();
    setup_error();
    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        fee: 10.into(),
        operation: Operation::CreditOnError,
        ..simple_transfer()
    }
    .double_step();

    terminal.transfer(transfer, 1).await.unwrap_err();
    assert_eq!(TestBalances::balance_of(alice()), 1000);
}

#[tokio::test]
async fn credit_on_error_second_stage_failed() {
    let mut terminal = init_test();
    let counter = Arc::new(AtomicUsize::new(0));

    register_virtual_responder(
        token_principal(),
        "icrc1_transfer",
        move |_: (TransferArg,)| {
            let count = counter.fetch_add(1, Ordering::Relaxed);
            if count == 0 {
                Ok::<Nat, TransferError>(Nat::from(1))
            } else {
                Err::<Nat, TransferError>(TransferError::TemporarilyUnavailable)
            }
        },
    );

    let transfer = Transfer {
        caller: alice(),
        amount: 1000.into(),
        fee: 10.into(),
        operation: Operation::CreditOnError,
        ..simple_transfer()
    }
    .double_step();

    terminal.transfer(transfer, 1).await.unwrap_err();
    assert_eq!(TestBalances::balance_of(alice()), 0);
    assert_eq!(ForRecoveryList::<0>.take_all().len(), 1);
}

#[tokio::test]
async fn recover_first_stage() {
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
    }
    .double_step();

    terminal.transfer(transfer, 3).await.unwrap_err();

    setup_success(1);

    let results = terminal.recover_all().await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(Nat::from(1)));
    assert_eq!(TestBalances::balance_of(alice()), 980);
    assert_eq!(ForRecoveryList::<0>.take_all().len(), 0);
}

#[tokio::test]
async fn recover_second_stage() {
    let mut terminal = init_test();
    let counter = Arc::new(AtomicUsize::new(0));
    register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
        if counter.fetch_add(1, Ordering::Relaxed) == 0 {
            let response: Result<Nat, TransferError> = Ok(Nat::from(1));
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
    }
    .double_step();

    terminal.transfer(transfer, 3).await.unwrap_err();

    setup_success(2);
    let results = terminal.recover_all().await;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(Nat::from(2)));
    assert_eq!(TestBalances::balance_of(alice()), 980);
    assert_eq!(ForRecoveryList::<0>.take_all().len(), 0);
}

fn setup_recovery_responses(
    interim_acc: Account,
    interim_balance: u128,
    final_tx_id: u128,
) -> Arc<AtomicBool> {
    let is_called = Arc::new(AtomicBool::new(false));
    let is_called_clone = is_called.clone();
    register_virtual_responder::<_, _, Nat>(
        token_principal(),
        "icrc1_balance_of",
        move |(account,): (Account,)| {
            assert_eq!(account, interim_acc);
            is_called.store(true, Ordering::Relaxed);
            interim_balance.into()
        },
    );

    setup_success(final_tx_id);

    is_called_clone
}

#[tokio::test]
async fn recover_first_stage_old_zero_balance() {
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
    }
    .double_step();

    let interim_acc = transfer.interim_acc().unwrap();
    terminal.transfer(transfer, 3).await.unwrap_err();
    let ctx = init_context();
    ctx.add_time(10u64.pow(9) * 60 * 60 * 24);

    let was_called = setup_recovery_responses(interim_acc, 0, 3);
    let results = terminal.recover_all().await;
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0],
        Err(PaymentError::TransferFailed(TransferFailReason::Unknown))
    );
    assert_eq!(TestBalances::balance_of(alice()), 0);
    assert_eq!(ForRecoveryList::<0>.take_all().len(), 0);
    assert!(was_called.load(Ordering::Relaxed));
}

#[tokio::test]
async fn recover_first_stage_old_non_zero_balance() {
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
    }
    .double_step();

    let interim_acc = transfer.interim_acc().unwrap();
    terminal.transfer(transfer, 3).await.unwrap_err();
    let ctx = init_context();
    ctx.add_time(10u64.pow(9) * 60 * 60 * 24);

    let was_called = setup_recovery_responses(interim_acc, 990, 3);
    let results = terminal.recover_all().await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(Nat::from(3)));
    assert_eq!(TestBalances::balance_of(alice()), 980);
    assert_eq!(ForRecoveryList::<0>.take_all().len(), 0);
    assert!(was_called.load(Ordering::Relaxed));
}

#[tokio::test]
async fn recover_second_stage_old_non_zero_balance() {
    let mut terminal = init_test();

    let call_counter = Arc::new(AtomicUsize::new(0));
    register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
        if call_counter.fetch_add(1, Ordering::Relaxed) == 0 {
            let response: Result<Nat, TransferError> = Ok(Nat::from(1));
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
    }
    .double_step();

    let interim_acc = transfer.interim_acc().unwrap();
    terminal.transfer(transfer, 3).await.unwrap_err();
    let ctx = init_context();
    ctx.add_time(10u64.pow(9) * 60 * 60 * 24);

    let was_called = setup_recovery_responses(interim_acc, 990, 3);
    let results = terminal.recover_all().await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(Nat::from(3)));
    assert_eq!(TestBalances::balance_of(alice()), 980);
    assert_eq!(ForRecoveryList::<0>.take_all().len(), 0);
    assert!(was_called.load(Ordering::Relaxed));
}

#[tokio::test]
async fn recover_second_stage_old_zero_balance() {
    let mut terminal = init_test();

    let call_counter = Arc::new(AtomicUsize::new(0));
    register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
        if call_counter.fetch_add(1, Ordering::Relaxed) == 0 {
            let response: Result<Nat, TransferError> = Ok(Nat::from(1));
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
    }
    .double_step();

    let interim_acc = transfer.interim_acc().unwrap();
    terminal.transfer(transfer, 3).await.unwrap_err();
    let ctx = init_context();
    ctx.add_time(10u64.pow(9) * 60 * 60 * 24);

    let was_called = setup_recovery_responses(interim_acc, 0, 3);
    let results = terminal.recover_all().await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(Nat::from(UNKNOWN_TX_ID)));
    assert_eq!(TestBalances::balance_of(alice()), 980);
    assert_eq!(ForRecoveryList::<0>.take_all().len(), 0);
    assert!(was_called.load(Ordering::Relaxed));
}

#[tokio::test]
async fn recover_multiple_transfers() {
    const TRANSFER_COUNT: usize = 37;
    const SUCCESSFUL_COUNT: usize = 3;
    const PARTIALLY_SUCCESSFUL_COUNT: usize = 5;

    let mut terminal = init_test();

    let call_counter = Arc::new(AtomicUsize::new(0));
    let transaction_counter = Arc::new(AtomicUsize::new(1));
    register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
        let call_number = call_counter.fetch_add(1, Ordering::Relaxed);
        if call_number < SUCCESSFUL_COUNT * 2 {
            println!("ok");
            let tx_id = transaction_counter.fetch_add(1, Ordering::Relaxed) as u128;
            let response: Result<Nat, TransferError> = Ok(Nat::from(tx_id));
            let response_bytes = Encode!(&response).unwrap();
            Ok(response_bytes)
        } else if call_number < SUCCESSFUL_COUNT * 2 + PARTIALLY_SUCCESSFUL_COUNT * 2
            && transaction_counter.load(Ordering::Relaxed)
                < SUCCESSFUL_COUNT * 2 + PARTIALLY_SUCCESSFUL_COUNT
            && call_number % 2 == 0
        {
            println!("kinda ok");
            let tx_id = transaction_counter.fetch_add(1, Ordering::Relaxed) as u128;
            let response: Result<Nat, TransferError> = Ok(Nat::from(tx_id));
            let response_bytes = Encode!(&response).unwrap();
            Ok(response_bytes)
        } else {
            println!("err");
            Err((RejectionCode::SysTransient, "recoverable".into()))
        }
    });

    let context = init_context();
    let mut successes = 0;
    let mut errors = 0;
    for _ in 0..TRANSFER_COUNT {
        let transfer = Transfer {
            caller: alice(),
            amount: 1000.into(),
            fee: 10.into(),
            operation: Operation::CreditOnSuccess,
            ..simple_transfer()
        }
        .double_step();

        match terminal.transfer(transfer, 1).await {
            Ok(_) => successes += 1,
            Err(_) => errors += 1,
        }

        context.add_time(10u64.pow(9) * 60 * 60);
    }

    assert_eq!(successes, SUCCESSFUL_COUNT);
    assert_eq!(errors, TRANSFER_COUNT - SUCCESSFUL_COUNT);

    assert_eq!(
        TestBalances::balance_of(alice()),
        980 * SUCCESSFUL_COUNT as i128
    );
    assert_eq!(ForRecoveryList::<0>.list().len(), errors);

    register_virtual_responder::<_, _, Nat>(
        token_principal(),
        "icrc1_balance_of",
        move |(_,): (Account,)| 990.into(),
    );

    setup_success(123);
    let results = terminal.recover_all().await;
    assert_eq!(results.len(), errors);
    assert!(results.iter().all(|v| v.is_ok()));

    assert_eq!(
        TestBalances::balance_of(alice()),
        980 * TRANSFER_COUNT as i128
    );
    assert_eq!(ForRecoveryList::<0>.take_all().len(), 0);
}
