use candid::{CandidType, Deserialize, Nat};
use error::InternalPaymentError;
use ic_exports::ic_icrc1::Account;
use ic_exports::ic_kit::ic;
use ic_exports::Principal;
use ic_helpers::tokens::Tokens128;

use crate::error::Result;

pub mod error;
mod icrc1;
pub mod recovery_list;
mod token_terminal;
mod transfer;

pub use token_terminal::*;
pub use transfer::*;

type Timestamp = u64;
type TxId = Nat;

pub trait Balances {
    fn credit(&mut self, recepient: Principal, amount: Tokens128) -> Result<Tokens128>;
}

#[derive(CandidType, Debug, Deserialize, Clone)]
pub struct TokenConfiguration {
    pub fee: Tokens128,
    pub minting_account: Account,
}

impl TokenConfiguration {
    fn get_fee(&self, transfer: &Transfer) -> Tokens128 {
        if transfer.from() == self.minting_account || transfer.to() == self.minting_account {
            Tokens128::ZERO
        } else {
            self.fee
        }
    }
}
