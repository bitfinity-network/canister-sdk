use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use candid::Nat;
use ic_canister::{register_raw_virtual_responder, register_virtual_responder};
use ic_exports::ic_kit::RejectCode;
use ic_exports::icrc_types::icrc1::transfer::{TransferArg, TransferError};
use ic_payments::error::{PaymentError, RecoveryDetails, TransferFailReason};
use ic_payments::recovery_list::{RecoveryList, StableRecoveryList};

use crate::common::{init_test, setup_success, simple_transfer, token_principal};

pub mod common;

fn make_error(code: RejectCode, message: &str) -> ic_exports::ic_cdk::call::Error {
    ic_exports::ic_cdk::call::Error::CallRejected(
        ic_exports::ic_cdk::call::CallRejected::with_rejection(code as u32, message.to_string()),
    )
}

#[tokio::test]
async fn successful_transfer() {
    let mut terminal = init_test();
    setup_success(1);

    let result = terminal.transfer(simple_transfer(), 1).await;
    assert_eq!(result, Ok(1u64.into()));
}

#[tokio::test]
async fn token_canister_does_not_exist() {
    let mut terminal = init_test();
    let result = terminal.transfer(simple_transfer(), 1).await;
    assert_eq!(
        result,
        Err(PaymentError::TransferFailed(TransferFailReason::NotFound))
    );
}

#[tokio::test]
async fn token_canister_rejects_request() {
    let mut terminal = init_test();
    register_raw_virtual_responder(token_principal(), "icrc1_transfer", |_| {
        // Token canister trapped or didn't respond
        Err(make_error(RejectCode::CanisterError, "trap"))
    });

    let result = terminal.transfer(simple_transfer(), 1).await;
    assert_eq!(
        result,
        Err(PaymentError::TransferFailed(
            TransferFailReason::TokenPanic("trap".to_string())
        ))
    );
}

#[tokio::test]
async fn ic_maybe_failed_codes() {
    let mut terminal = init_test();
    let recoverable_codes = vec![
        RejectCode::SysFatal,
        RejectCode::SysTransient,
        RejectCode::SysUnknown,
        RejectCode::CanisterReject,
    ];

    for code in recoverable_codes {
        register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
            // Token canister trapped or didn't respond
            Err(make_error(code, "recoverable"))
        });

        let result = terminal.transfer(simple_transfer(), 1).await;
        assert_eq!(
            result,
            Err(PaymentError::Recoverable(RecoveryDetails::IcError))
        );
    }
}

#[tokio::test]
async fn token_rejects_transaction() {
    let mut terminal = init_test();
    register_virtual_responder(token_principal(), "icrc1_transfer", |_: (TransferArg,)| {
        Err::<Nat, TransferError>(TransferError::InsufficientFunds {
            balance: 100u64.into(),
        })
    });

    let result = terminal.transfer(simple_transfer(), 1).await;
    assert_eq!(
        result,
        Err(PaymentError::TransferFailed(TransferFailReason::Rejected(
            TransferError::InsufficientFunds {
                balance: 100u64.into()
            }
        )))
    );
}

#[tokio::test]
async fn token_rejects_with_bad_fee() {
    let mut terminal = init_test();
    register_virtual_responder(token_principal(), "icrc1_transfer", |_: (TransferArg,)| {
        Err::<Nat, TransferError>(TransferError::BadFee {
            expected_fee: 10u64.into(),
        })
    });

    let result = terminal.transfer(simple_transfer(), 1).await;
    assert_eq!(result, Err(PaymentError::BadFee(10u64.into())));
}

#[tokio::test]
async fn token_rejects_with_duplicate() {
    let mut terminal = init_test();
    register_virtual_responder(token_principal(), "icrc1_transfer", |_: (TransferArg,)| {
        Err::<Nat, TransferError>(TransferError::Duplicate {
            duplicate_of: 3u64.into(),
        })
    });

    let result = terminal.transfer(simple_transfer(), 1).await;
    assert_eq!(
        result,
        Err(PaymentError::TransferFailed(TransferFailReason::Rejected(
            TransferError::Duplicate {
                duplicate_of: 3u64.into(),
            }
        )))
    );
}

#[tokio::test]
async fn no_retries_on_error() {
    let mut terminal = init_test();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    register_virtual_responder(
        token_principal(),
        "icrc1_transfer",
        move |_: (TransferArg,)| {
            counter.fetch_add(1, Ordering::Relaxed);
            Err::<Nat, TransferError>(TransferError::InsufficientFunds {
                balance: 100u64.into(),
            })
        },
    );

    terminal.transfer(simple_transfer(), 5).await.unwrap_err();
    assert_eq!(counter_clone.load(Ordering::Relaxed), 1);
    assert_eq!(StableRecoveryList::<0>.take_all().len(), 0);
}

#[tokio::test]
async fn retries_count() {
    let mut terminal = init_test();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    register_raw_virtual_responder(token_principal(), "icrc1_transfer", move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
        Err(make_error(RejectCode::SysTransient, "recoverable"))
    });

    terminal.transfer(simple_transfer(), 5).await.unwrap_err();
    assert_eq!(counter_clone.load(Ordering::Relaxed), 5);
    assert_eq!(StableRecoveryList::<0>.take_all().len(), 1);
}
