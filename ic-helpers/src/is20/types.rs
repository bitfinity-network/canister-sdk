use candid::{CandidType, Deserialize};

#[derive(CandidType, Debug, Eq, PartialEq, Deserialize)]
pub enum TxError {
    InsufficientBalance,
    InsufficientAllowance,
    Unauthorized,
    AmountTooSmall,
    FeeExceededLimit,
    NotificationFailed,
    AlreadyNotified,
    TransactionDoesNotExist,
}
