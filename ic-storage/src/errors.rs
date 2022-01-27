use thiserror::Error;
use ic_cdk::api::stable::StableMemoryError;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Insufficient space available")]
    InsufficientSpace,
    
    #[error("Stable memory error")]
    StableMemory,

    #[error("Attempted downgrade, or trying to load a version older than what is currently stored")]
    AttemptedDowngrade,

    #[error("Candid error: {0}")]
    Candid(#[from] ic_cdk::export::candid::Error),
}

// Required because `StableMemoryError` doesn't implement Debug
impl From<StableMemoryError> for Error {
    fn from(_: StableMemoryError) -> Self {
        Self::StableMemory
    }
}
