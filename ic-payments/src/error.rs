use candid::{CandidType, Deserialize, Nat};
use ic_exports::ic_cdk::call::{
    CandidDecodeFailed, Error as CallError, InsufficientLiquidCycleBalance,
};
use ic_exports::ic_kit::RejectCode;
use ic_exports::icrc_types::icrc1::transfer::TransferError;
use thiserror::Error;

use crate::BalanceError;

pub type Result<T> = std::result::Result<T, InternalPaymentError>;

/// Reason for a transfer to fail
#[derive(Debug, PartialEq, Eq, Clone, CandidType, Deserialize, Error)]
pub enum TransferFailReason {
    #[error("token canister does not exist or doesn't follow the ICRC-1 standard")]
    NotFound,

    #[error("token canister panicked or didn't respond: {0}")]
    TokenPanic(String),

    #[error("transfer request was rejected: {0:?}")]
    Rejected(TransferError),

    #[error("transaction is too old to be executed")]
    TooOld,

    #[error("unknown")]
    Unknown,
}

/// Error while executing a transfer.
#[derive(Debug, PartialEq, Eq, Clone, CandidType, Deserialize, Error)]
pub enum PaymentError {
    /// Requested transfer parameters are invalid.
    ///
    /// This error means that the terminal didn't even attempt to perform the transaction. No
    /// further requests with the same parameters would be successful.
    #[error("invalid transfer parameters: {0}")]
    InvalidParameters(ParametersError),

    /// Transaction was attempted but rejected by the token canister. It's unlikely that further
    /// requests with the same parameters would be successful in the current state.
    ///
    /// When this error is returned it's guaranteed that the attempted transaction was not
    /// executed, so there's no need for recovery. After the reason of failure is dealt with the
    /// same transfer can be attempted again with the same parameters.
    #[error("transfer failed: {0}")]
    TransferFailed(TransferFailReason),

    /// Transaction was attempted but the token transfer fee configuration stored in the terminal
    /// was incorrect, so the token canister rejected the transaction.
    ///
    /// Calling canister must adjust its configuration and then can attempt the same transfer
    /// again.
    #[error("transfer fee setting was not same as token fee configuration {0}")]
    BadFee(Nat),

    /// Unknown error happened while attempting the transfer. The terminal cannot be sure that the
    /// transaction was not executed by the token canister, so the transfer is added to the `for
    /// recovery` list.
    ///
    /// Recovery of the transfer may be attempted by the terminal recovery mechanism.
    #[error("IC error occurred, the transaction can potentially be recovered: {0:?}")]
    Recoverable(RecoveryDetails),

    #[error("caller's balance is not enough to perform the operation")]
    InsufficientFunds,

    #[error("unrecoverable error: {0}")]
    Fatal(String),
}

/// Reason for the transfer failure.
#[derive(Debug, CandidType, Deserialize, PartialEq, Eq, Clone)]
pub enum RecoveryDetails {
    /// IC error occurred that doesn't guarantee a specific state of the request. After the IC
    /// deals with the reason of the error, the recovery can be attempted again.
    IcError,

    /// Second stage transfer returned `BadFee` error. The token terminal should update it's token
    /// configuration and attempt to recover the transfer.
    BadFee(Nat),
}

#[derive(Debug, Error, CandidType, Deserialize, PartialEq)]
pub enum InternalPaymentError {
    #[error("transfer error: {0:?}")]
    TransferFailed(TransferFailReason),

    #[error("wrong fee")]
    WrongFee(Nat),

    #[error("maybe failed")]
    MaybeFailed,

    #[error("requested transfer has invalid parameters: {0:?}")]
    InvalidParameters(#[from] ParametersError),

    #[error("value overflow")]
    Overflow,

    #[error("call perform failed")]
    CallPerformFailed,

    #[error("{0}")]
    CandidDecodeFailed(String),

    #[error("insufficient liquid cycle balance; available {available}, required {required}")]
    InsufficientLiquidCycleBalance { available: u128, required: u128 },
}

/// Invalid transfer parameters.
#[derive(Debug, CandidType, Deserialize, PartialEq, Eq, Clone, Error)]
pub enum ParametersError {
    #[error(
        "amount to transfer {actual} is smaller than minimum possible value {minimum_required}"
    )]
    AmountTooSmall { minimum_required: Nat, actual: Nat },

    #[error("target account cannot be equal to the source account")]
    TargetAccountInvalid,
}

impl From<CandidDecodeFailed> for InternalPaymentError {
    fn from(value: CandidDecodeFailed) -> Self {
        Self::CandidDecodeFailed(value.to_string())
    }
}

impl From<CallError> for InternalPaymentError {
    fn from(err: CallError) -> Self {
        match err {
            CallError::CallPerformFailed(_) => Self::CallPerformFailed,
            CallError::CallRejected(err) => {
                match err.reject_code().unwrap_or(RejectCode::SysUnknown) {
                    // Token canister doesn't exist or doesn't have the `icrc1_transfer` method
                    RejectCode::DestinationInvalid => {
                        Self::TransferFailed(TransferFailReason::NotFound)
                    }
                    // Token canister panicked or didn't respond at all. This can happen if the token
                    // canister is out of cycles or is undergoing an upgrade.
                    RejectCode::CanisterError => Self::TransferFailed(
                        TransferFailReason::TokenPanic(err.reject_message().to_string()),
                    ),
                    // IC error or violation of IC specification. Since we don't know for sure how to deal
                    // with this in advance, we treat them as potentially recoverable errors, hoping that
                    // in the future IC will recover and start returning something sensible.
                    RejectCode::SysUnknown
                    | RejectCode::SysFatal
                    | RejectCode::SysTransient
                    | RejectCode::CanisterReject => Self::MaybeFailed,
                }
            }
            CallError::InsufficientLiquidCycleBalance(InsufficientLiquidCycleBalance {
                available,
                required,
            }) => Self::InsufficientLiquidCycleBalance {
                available,
                required,
            },
            CallError::CandidDecodeFailed(err) => Self::CandidDecodeFailed(err.to_string()),
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
            TransferError::BadFee { expected_fee } => Self::WrongFee(expected_fee),
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
            InternalPaymentError::InvalidParameters(v) => Self::InvalidParameters(v),
            InternalPaymentError::CandidDecodeFailed(v) => Self::Fatal(v),
            InternalPaymentError::CallPerformFailed => Self::Fatal("call perform failed".into()),
            InternalPaymentError::InsufficientLiquidCycleBalance { .. } => Self::InsufficientFunds,
        }
    }
}

impl From<BalanceError> for PaymentError {
    fn from(value: BalanceError) -> Self {
        match value {
            BalanceError::InsufficientFunds => PaymentError::InsufficientFunds,
            BalanceError::Fatal(v) => PaymentError::Fatal(v),
        }
    }
}
