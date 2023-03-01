use candid::Nat;
use ic_exports::ic_icrc1::Account;
use ic_exports::ic_kit::mock_principals::alice;

mod common;
use common::*;
use ic_payments::recovery_list::{RecoveryList, StableRecoveryList};
use ic_payments::{TokenConfiguration, Transfer};

#[tokio::test]
async fn deposit_with_success() {
    let mut terminal = init_test();
    setup_success(1);

    let (tx_id, amount) = terminal.deposit(alice(), 1000.into()).await.unwrap();
    assert_eq!(tx_id, Nat::from(1));
    assert_eq!(amount, 990.into());
    assert_eq!(TestBalances::balance_of(alice()), 990);
}

#[tokio::test]
async fn deposit_with_error() {
    let mut terminal = init_test();
    setup_error();

    terminal.deposit(alice(), 1000.into()).await.unwrap_err();
    assert_eq!(TestBalances::balance_of(alice()), 0);
}

#[tokio::test]
async fn withdraw_with_success() {
    let mut terminal = init_test();
    setup_success(1);

    let (tx_id, amount) = terminal.withdraw(alice(), 1000.into()).await.unwrap();
    assert_eq!(tx_id, Nat::from(1));
    assert_eq!(amount, 980.into());
    assert_eq!(TestBalances::balance_of(alice()), -1000);
}

#[tokio::test]
async fn withdraw_with_error() {
    let mut terminal = init_test();
    setup_error();

    terminal.withdraw(alice(), 1000.into()).await.unwrap_err();
    assert_eq!(TestBalances::balance_of(alice()), 0);
}

#[test]
fn update_fees() {
    let mut terminal = init_test();
    let transfer = simple_transfer().with_fee(15.into());

    StableRecoveryList::<0>.push(transfer);

    terminal.set_fee(20.into());

    assert_eq!(StableRecoveryList::<0>.list()[0].fee, 20.into());
}

#[test]
fn update_minting_account() {
    let mut terminal = init_test();
    let token_config = TokenConfiguration {
        principal: token_principal(),
        fee: 10.into(),
        minting_account: minting_account(),
    };

    let transfer = Transfer::new(&token_config, alice(), minting_account(), None, 1000.into());

    assert_eq!(transfer.fee, 0.into());
    assert_eq!(transfer.effective_fee().unwrap(), 0.into());

    StableRecoveryList::<0>.push(transfer);

    terminal.set_minting_account(Account {
        owner: alice().into(),
        subaccount: Some([12; 32]),
    });

    assert_eq!(StableRecoveryList::<0>.list()[0].fee, 10.into());
    assert_eq!(
        StableRecoveryList::<0>.list()[0].effective_fee().unwrap(),
        10.into()
    );
}
