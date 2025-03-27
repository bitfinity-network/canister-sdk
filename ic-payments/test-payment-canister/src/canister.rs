//! This is a canister for integration testing of payment terminal. It provides
//! api to the main terminal methods to test communication between the terminal
//! and ICRC-1 token.
use std::collections::HashMap;

use candid::Nat;
use ic_canister::{generate_idl, init, update, Canister, Idl, PreUpdate};
use ic_exports::candid::{CandidType, Deserialize, Principal};
use ic_exports::ic_kit::ic;
use ic_exports::icrc_types::icrc1::account::Account;
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
                    fee: 0u64.into(),
                    minting_account: Principal::management_canister().into(),
                },
                TestBalances::default(),
            ),
        }
    }
}

#[derive(Debug, Default, CandidType, Deserialize)]
struct TestBalances {
    map: HashMap<Principal, Nat>,
}

impl Balances for TestBalances {
    fn credit(&mut self, account_owner: Principal, amount: Nat) -> Result<Nat, BalanceError> {
        let entry = self.map.entry(account_owner).or_default();
        *entry = entry.clone() + amount;
        Ok(entry.clone())
    }

    fn debit(&mut self, account_owner: Principal, amount: Nat) -> Result<Nat, BalanceError> {
        let entry = self.map.entry(account_owner).or_default();
        *entry = entry.clone() - amount;
        Ok(entry.clone())
    }
}

#[derive(Debug, Canister)]
pub struct PaymentCanister {
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
            fee: 0u64.into(),
            minting_account: Account {
                owner: Principal::management_canister(),
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
    async fn deposit(&self, amount: Nat) -> Result<(Nat, Nat), PaymentError> {
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
    async fn withdraw(&self, amount: Nat) -> Result<(Nat, Nat), PaymentError> {
        PaymentState::get()
            .borrow_mut()
            .terminal
            .withdraw(ic::caller(), amount)
            .await
    }

    #[update]
    async fn get_balance(&self) -> (Nat, Nat) {
        let state = PaymentState::get();
        let terminal = &state.borrow().terminal;
        let token = terminal.token_config().principal;
        let caller = ic::caller();
        ic::print(format!("Getting balance for {caller}"));

        let local_balance = terminal
            .balances()
            .map
            .get(&caller)
            .cloned()
            .unwrap_or_default();

        let token_canister_balance = ic_payments::icrc1::get_icrc1_balance(
            token,
            &Account {
                owner: ic::id(),
                subaccount: None,
            },
        )
        .await
        .unwrap();

        (local_balance, token_canister_balance)
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}
