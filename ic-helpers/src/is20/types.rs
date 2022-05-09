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
    BadFee { expected_fee: u64 },
    InsufficientFunds { balance: u64 },
    TxTooOld { allowed_window_nanos: u64 },
    TxCreatedInFuture,
    TxDuplicate { duplicate_of: u64 },
}
