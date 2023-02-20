use candid::{CandidType, Deserialize};
use ic_exports::ic_icrc1::endpoints::TransferError;
use ic_exports::ic_kit::RejectionCode;
use ic_helpers::tokens::Tokens128;
use thiserror::Error;

use crate::TxId;

pub type Result<T> = std::result::Result<T, PaymentError>;

#[derive(Debug, Error, CandidType, Deserialize)]
pub enum PaymentError {
    #[error("transfer error: {0:?}")]
    TransferFailed(TransferError),

    #[error("wrong fee")]
    WrongFee(Tokens128),

    #[error("maybe failed")]
    MaybeFailed,

    #[error("duplicate")]
    Duplicate(TxId),

    #[error("recoverable")]
    Recoverable,

    #[error("stable memory error: {0}")]
    StableMemory(String),
}

impl From<(RejectionCode, String)> for PaymentError {
    fn from(_: (RejectionCode, String)) -> Self {
        todo!()
    }
}

impl From<TransferError> for PaymentError {
    fn from(_: TransferError) -> Self {
        todo!()
    }
}
