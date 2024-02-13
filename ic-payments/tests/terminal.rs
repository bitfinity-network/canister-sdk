use common::*;
use ic_exports::ic_kit::mock_principals::alice;
use ic_exports::icrc_types::icrc1::account::Account;
use ic_payments::recovery_list::{RecoveryList, StableRecoveryList};
use ic_payments::{Balances, TokenConfiguration, Transfer};

pub mod common;

#[tokio::test]
async fn deposit_with_success() {
    let mut terminal = init_test();
    setup_success(1);

    let (tx_id, amount) = terminal.deposit(alice(), 1000u64.into()).await.unwrap();
    assert_eq!(tx_id, 1u64);
    assert_eq!(amount, 990u64);
    assert_eq!(TestBalances::balance_of(alice()), 990u64);
}

#[tokio::test]
async fn deposit_with_error() {
    let mut terminal = init_test();
    setup_error();

    terminal.deposit(alice(), 1000u64.into()).await.unwrap_err();
    assert_eq!(TestBalances::balance_of(alice()), 0u64);
}

#[tokio::test]
async fn withdraw_with_success() {
    let mut terminal = init_test();
    setup_success(1);
    TestBalances.credit(alice(), 3000u64.into()).unwrap();

    let (tx_id, amount) = terminal.withdraw(alice(), 1000u64.into()).await.unwrap();
    assert_eq!(tx_id, 1u64);
    assert_eq!(amount, 980u64);
    assert_eq!(TestBalances::balance_of(alice()), 2000u64);
}

#[tokio::test]
async fn withdraw_with_error() {
    let mut terminal = init_test();
    setup_error();
    TestBalances.credit(alice(), 3000u64.into()).unwrap();

    terminal
        .withdraw(alice(), 1000u64.into())
        .await
        .unwrap_err();
    assert_eq!(TestBalances::balance_of(alice()), 3000u64);
}

#[test]
fn update_fees() {
    let mut terminal = init_test();
    let transfer = simple_transfer().with_fee(15u64.into());

    StableRecoveryList::<0>.push(transfer);

    terminal.set_fee(20u64.into());

    assert_eq!(StableRecoveryList::<0>.list()[0].fee, 20u64);
}

#[test]
fn update_minting_account() {
    let mut terminal = init_test();
    let token_config = TokenConfiguration {
        principal: token_principal(),
        fee: 10u64.into(),
        minting_account: minting_account(),
    };

    let transfer = Transfer::new(
        &token_config,
        alice(),
        minting_account(),
        None,
        1000u64.into(),
    );

    assert_eq!(transfer.fee, 0u64);
    assert_eq!(transfer.effective_fee(), 0u64);

    StableRecoveryList::<0>.push(transfer);

    terminal.set_minting_account(Account {
        owner: alice(),
        subaccount: Some([12; 32]),
    });

    assert_eq!(StableRecoveryList::<0>.list()[0].fee, 10u64);
    assert_eq!(StableRecoveryList::<0>.list()[0].effective_fee(), 10u64);
}
