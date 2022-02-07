use ic_cdk::export::candid::{CandidType, Deserialize};
use thiserror::Error;

#[derive(Debug, Error, CandidType, Deserialize, PartialEq)]
pub enum FactoryError {
    #[error("request to the ledger failed: {0}")]
    LedgerError(String),

    #[error("not enough cycles provided to create a canister. Provided: {0}. Required: {1}")]
    NotEnoughCycles(u64, u64),

    #[error("not enough ICP provided to create a canister. Provided: {0}. Required: {1}")]
    NotEnoughIcp(u64, u64),
}
