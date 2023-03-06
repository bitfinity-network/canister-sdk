use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use ic_helpers::tokens::Tokens128;
use thiserror::Error;

/// Error while trying to change user's balance.
#[derive(Debug, PartialEq, Error)]
pub enum BalanceError {
    #[error("user balance is less then the requested debit amount")]
    InsufficientFunds,

    #[error("unrecoverable error")]
    Fatal(String),
}

/// Interface for handling the canister balances storage.
pub trait Balances {
    /// Increase the `account_owner`'s balance by the given `amount`.
    fn credit(
        &mut self,
        account_owner: Principal,
        amount: Tokens128,
    ) -> Result<Tokens128, BalanceError>;

    /// Decrease the `account_owners`'s balance by the given `amount`.
    fn debit(
        &mut self,
        account_owner: Principal,
        amount: Tokens128,
    ) -> Result<Tokens128, BalanceError>;
}

impl<T: Balances> Balances for Rc<RefCell<T>> {
    fn credit(
        &mut self,
        account_owner: Principal,
        amount: Tokens128,
    ) -> Result<Tokens128, crate::BalanceError> {
        self.borrow_mut().credit(account_owner, amount)
    }

    fn debit(
        &mut self,
        account_owner: Principal,
        amount: Tokens128,
    ) -> Result<Tokens128, crate::BalanceError> {
        self.borrow_mut().debit(account_owner, amount)
    }
}
