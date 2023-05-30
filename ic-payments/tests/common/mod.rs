use std::cell::RefCell;

use candid::{Nat, Principal};
use ic_canister::register_virtual_responder;
use ic_exports::ic_kit::mock_principals::alice;
use ic_exports::ic_kit::MockContext;
use ic_exports::icrc1::account::Account;
use ic_exports::icrc1::transfer::{TransferArg, TransferError};
use ic_payments::recovery_list::StableRecoveryList;
use ic_payments::{BalanceError, Balances, TokenConfiguration, TokenTerminal, Transfer};

pub enum BalanceOperation {
    Credit(Principal, Nat),
    Debit(Principal, Nat),
}

impl BalanceOperation {
    fn of(&self) -> Principal {
        match self {
            Self::Credit(owner, _) => *owner,
            Self::Debit(owner, _) => *owner,
        }
    }

    fn perform(&self, with: Nat) -> Nat {
        match self {
            Self::Credit(_, amount) => with + amount.clone(),
            Self::Debit(_, amount) => with - amount.clone(),
        }
    }
}

pub struct TestBalances;

impl Balances for TestBalances {
    fn credit(&mut self, recipient: Principal, amount: Nat) -> Result<Nat, BalanceError> {
        BALANCES.with(|v| {
            v.borrow_mut()
                .push(BalanceOperation::Credit(recipient, amount.clone()))
        });

        Ok(amount)
    }

    fn debit(&mut self, account_owner: Principal, amount: Nat) -> Result<Nat, BalanceError> {
        BALANCES.with(|v| {
            v.borrow_mut()
                .push(BalanceOperation::Debit(account_owner, amount.clone()))
        });

        Ok(amount)
    }
}

impl TestBalances {
    pub fn balance_of(principal: Principal) -> Nat {
        BALANCES.with(|v| {
            v.borrow()
                .iter()
                .filter(|entry| entry.of() == principal)
                .fold(0.into(), |acc, item| item.perform(acc))
        })
    }
}

thread_local! {
    static BALANCES: RefCell<Vec<BalanceOperation>> = RefCell::new(vec![]);
}

pub fn token_principal() -> Principal {
    Principal::from_slice(&[1; 29])
}

pub fn minting_account() -> Account {
    Account {
        owner: Principal::from_slice(&[3; 29]).into(),
        subaccount: None,
    }
}

pub fn token_config() -> TokenConfiguration {
    TokenConfiguration {
        principal: token_principal(),
        fee: 100.into(),
        minting_account: minting_account(),
    }
}

pub fn this_principal() -> Principal {
    Principal::from_slice(&[2; 29])
}

pub fn simple_transfer() -> Transfer {
    let to = Account {
        owner: alice().into(),
        subaccount: None,
    };
    Transfer::new(&token_config(), alice(), to, None, 1000.into())
}

pub fn init_context() -> &'static MockContext {
    MockContext::new().with_id(this_principal()).inject()
}

pub fn init_test() -> TokenTerminal<TestBalances, StableRecoveryList<0>> {
    BALANCES.with(|v| *v.borrow_mut() = vec![]);
    init_context();

    TokenTerminal::new(
        TokenConfiguration {
            principal: token_principal(),
            fee: 10.into(),
            minting_account: minting_account(),
        },
        TestBalances {},
    )
}

pub fn setup_success(tx_id: u128) {
    register_virtual_responder(
        token_principal(),
        "icrc1_transfer",
        move |_: (TransferArg,)| Ok::<Nat, TransferError>(tx_id.into()),
    );
}

pub fn setup_error() {
    register_virtual_responder(
        token_principal(),
        "icrc1_transfer",
        move |_: (TransferArg,)| {
            Err::<Nat, TransferError>(TransferError::InsufficientFunds { balance: 0.into() })
        },
    );
}
