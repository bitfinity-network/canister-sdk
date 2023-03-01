use candid::{CandidType, Deserialize};
use ic_exports::ic_icrc1::endpoints::TransferError;
use ic_exports::ic_kit::RejectionCode;
use ic_helpers::tokens::Tokens128;
use thiserror::Error;

use crate::TxId;

pub type Result<T> = std::result::Result<T, InternalPaymentError>;

#[derive(Debug, PartialEq, CandidType, Deserialize)]
pub enum TransferFailReason {
    /// Token canister does not exist or does not have `icrc1_transfer` method.
    NotFound,

    /// Token canister panicced or didn't respond.
    TokenPanic(String),

    /// Token canister rejected the request.
    Rejected(TransferError),

    TooOld,

    Unknown,
}

#[derive(Debug, PartialEq, CandidType, Deserialize)]
pub enum PaymentError {
    /// Requested transfer parameters are invalid.
    ///
    /// This error means that the terminal didn't even attempt to perform the transaction. No
    /// further requests with the same parameters would be successful.
    ///
    /// Possible errors could be:
    /// * transfer amount is smaller than the token transaction fee
    /// * `from` account does not belong to the calling canister
    InvalidParameters,

    /// Transaction was attempted but rejected by the token canister. It's unlikely that further
    /// requests with the same parameters would be successful in current state.
    ///
    /// When this error is returned it's guaranteed that the attempted transaction was not
    /// executed, so there's no need for recovery. After the reason of failure is dealt with the
    /// same transfer can be attempted again with the same parameters.
    TransferFailed(TransferFailReason),

    /// Transaction was attempted but the token transfer fee configuration stored in the terminal
    /// was incorrect, so the token canister rejected the transaction.
    ///
    /// Calling canister must adjust its configuration and then can attempt the same transfer
    /// again.
    BadFee(Tokens128),

    /// Unknown error happend while attempting the transfer. The terminal cannot be sure that the
    /// tranaction was not executed by the token canister, so the transfer is added to the `for
    /// recovery` list.
    ///
    /// Recovery of the transfer may be attempted by the terminal recovery mechanism.
    Recoverable(RecoveryDetails),

    Fatal(String),
}

#[derive(Debug, CandidType, Deserialize, PartialEq)]
pub enum RecoveryDetails {
    IcError,
    BadFee(Tokens128),
}

#[derive(Debug, Error, CandidType, Deserialize, PartialEq)]
pub enum InternalPaymentError {
    #[error("transfer error: {0:?}")]
    TransferFailed(TransferFailReason),

    #[error("wrong fee")]
    WrongFee(Tokens128),

    #[error("maybe failed")]
    MaybeFailed,

    #[error("stable memory error: {0}")]
    StableMemory(String),

    #[error("requested transfer has invalid parameters: {0:?}")]
    InvalidParameters(ParametersError),

    #[error("value overflow")]
    Overflow,

    #[error("unknown")]
    Unknown,
}

#[derive(Debug, CandidType, Deserialize, PartialEq)]
pub enum ParametersError {
    AmountTooSmall {
        minimum_required: Tokens128,
        actual: Tokens128,
    },
    NotOwner,
    TargetAccountInvalid,
    FeeTooLarge,
}

impl From<(RejectionCode, String)> for InternalPaymentError {
    fn from((code, message): (RejectionCode, String)) -> Self {
        match code {
            // Token canister doesn't exist or doesn't have `icrc1_transfer` method
            RejectionCode::DestinationInvalid => Self::TransferFailed(TransferFailReason::NotFound),
            // Token canister panicced or didn't respond at all
            RejectionCode::CanisterError => {
                Self::TransferFailed(TransferFailReason::TokenPanic(message))
            }
            RejectionCode::Unknown => todo!("{code:?}, {message}"),
            // IC error or violation of IC specification. Since we don't know for sure how to deal
            // with this in advance, we treat them as potentially recoverable errors, hoping that
            // in future IC will recover and start returning something sensible.
            RejectionCode::Unknown
            | RejectionCode::SysFatal
            | RejectionCode::SysTransient
            | RejectionCode::CanisterReject
            | RejectionCode::NoError => Self::MaybeFailed,
        }
    }
}

impl From<TransferError> for InternalPaymentError {
    fn from(err: TransferError) -> Self {
        match err {
            TransferError::InsufficientFunds { .. }
            | TransferError::TooOld
            | TransferError::BadBurn { .. }
            | TransferError::CreatedInFuture { .. }
            | TransferError::TemporarilyUnavailable
            | TransferError::Duplicate { .. }
            | TransferError::GenericError { .. } => {
                Self::TransferFailed(TransferFailReason::Rejected(err))
            }
            TransferError::BadFee { expected_fee } => {
                // todo: remove unwrap
                Self::WrongFee(Tokens128::from_nat(&expected_fee).unwrap())
            }
        }
    }
}

impl From<InternalPaymentError> for PaymentError {
    fn from(internal: InternalPaymentError) -> Self {
        match internal {
            InternalPaymentError::TransferFailed(reason) => Self::TransferFailed(reason),
            InternalPaymentError::MaybeFailed => Self::Recoverable(RecoveryDetails::IcError),
            InternalPaymentError::WrongFee(expected) => Self::BadFee(expected),
            InternalPaymentError::Overflow => Self::Fatal("token amount overflow".into()),
            _ => todo!("not handled error: {internal:?}"),
        }
    }
}
