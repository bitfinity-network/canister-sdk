use ic_exports::ic_cdk::export::candid::{CandidType, Deserialize};
use thiserror::Error;

#[derive(Debug, Error, CandidType, Deserialize)]
pub enum FactoryError {
    #[error("request to the ledger failed: {0}")]
    LedgerError(String),

    #[error("stable storage error: {0}")]
    StableStorageError(String),

    #[error("not enough cycles provided to create a canister. Provided: {0}. Required: {1}")]
    NotEnoughCycles(u64, u64),

    #[error("not enough ICP provided to create a canister. Provided: {0}. Required: {1}")]
    NotEnoughIcp(u64, u64),

    #[error("only the factory controller is allowed to call this method")]
    AccessDenied,

    #[error("canister is not in factory registry")]
    NotFound,

    #[error("canister management operation failed: {0}")]
    ManagementError(String),

    #[error("factory is not initialized properly: canister wasm not set")]
    CanisterWasmNotSet,

    #[error("factory state is locked due to another async operation running")]
    StateLocked,

    #[error("failed to create canister: {0}")]
    CanisterCreateFailed(String),

    #[error("factory error: {0}")]
    GenericError(String),
}
