//! This is a canister for integration testing of payment terminal. It provides
//! api to the main terminal methods to test communication between the terminal
//! and ICRC-1 token.
use std::collections::HashMap;

use candid::Nat;
use ic_canister::{init, update, Canister, PreUpdate};
use ic_exports::ic_base_types::PrincipalId;
use ic_exports::ic_cdk::export::candid::{CandidType, Deserialize, Principal};
use ic_exports::ic_icrc1::Account;
use ic_exports::ic_kit::ic;
use ic_helpers::tokens::Tokens128;
use ic_payments::error::PaymentError;
use ic_payments::icrc1::get_icrc1_configuration;
use ic_payments::{BalanceError, Balances, StableRecoveryList, TokenConfiguration, TokenTerminal};
use ic_storage::IcStorage;

#[derive(IcStorage)]
pub struct PaymentState {
    terminal: TokenTerminal<TestBalances, StableRecoveryList<0>>,
}

impl Default for PaymentState {
    fn default() -> Self {
        Self {
            terminal: TokenTerminal::new(
                TokenConfiguration {
                    principal: Principal::management_canister(),
                    fee: 0.into(),
                    minting_account: PrincipalId::from(Principal::management_canister()).into(),
                },
                TestBalances::default(),
            ),
        }
    }
}

#[derive(Debug, Default, CandidType, Deserialize)]
struct TestBalances {
    map: HashMap<Principal, Tokens128>,
}

impl Balances for TestBalances {
    fn credit(
        &mut self,
        account_owner: Principal,
        amount: Tokens128,
    ) -> Result<Tokens128, BalanceError> {
        ic::print(format!("Adding {amount} for {account_owner}"));
        let entry = self.map.entry(account_owner).or_default();
        *entry = (*entry + amount).unwrap();
        Ok(*entry)
    }

    fn debit(
        &mut self,
        account_owner: Principal,
        amount: Tokens128,
    ) -> Result<Tokens128, BalanceError> {
        let entry = self.map.entry(account_owner).or_default();
        *entry = (*entry - amount).ok_or(BalanceError::InsufficientFunds)?;
        Ok(*entry)
    }
}

#[derive(Debug, Canister)]
struct PaymentCanister {
    #[id]
    id: Principal,
}

impl PreUpdate for PaymentCanister {}

#[allow(clippy::await_holding_refcell_ref)]
impl PaymentCanister {
    #[init]
    pub async fn init(&self, token_canister: Principal) {
        let config = TokenConfiguration {
            principal: token_canister,
            fee: 0.into(),
            minting_account: Account {
                owner: Principal::management_canister().into(),
                subaccount: None,
            },
        };
        let terminal = TokenTerminal::new(config, TestBalances::default());

        PaymentState::get().replace(PaymentState { terminal });
    }

    #[update]
    async fn configure(&self) {
        let state = PaymentState::get();
        let terminal = &mut state.borrow_mut().terminal;

        let config = get_icrc1_configuration(terminal.token_config().principal)
            .await
            .unwrap();
        ic::print(format!("Token config: {config:?}"));
        terminal.set_fee(config.fee);
        terminal.set_minting_account(config.minting_account);
    }

    #[update]
    async fn deposit(&self, amount: Tokens128) -> Result<(Nat, Tokens128), PaymentError> {
        let caller = ic::caller();
        let result = PaymentState::get()
            .borrow_mut()
            .terminal
            .deposit(ic::caller(), amount)
            .await;

        ic::print(format!(
            "Balance: {:?}",
            PaymentState::get()
                .borrow()
                .terminal
                .balances()
                .map
                .get(&caller)
        ));
        ic::print(format!("{result:?}"));
        result
    }

    #[update]
    async fn withdraw(&self, amount: Tokens128) -> Result<(Nat, Tokens128), PaymentError> {
        PaymentState::get()
            .borrow_mut()
            .terminal
            .withdraw(ic::caller(), amount)
            .await
    }

    #[update]
    async fn get_balance(&self) -> (Tokens128, Tokens128) {
        let state = PaymentState::get();
        let terminal = &state.borrow().terminal;
        let token = terminal.token_config().principal;
        let caller = ic::caller();
        ic::print(format!("Getting balance for {caller}"));

        let local_balance = terminal
            .balances()
            .map
            .get(&caller)
            .copied()
            .unwrap_or_default();

        let token_canister_balance = ic_payments::icrc1::get_icrc1_balance(
            token,
            &Account {
                owner: ic::id().into(),
                subaccount: None,
            },
        )
        .await
        .unwrap();

        (local_balance, token_canister_balance)
    }
}
