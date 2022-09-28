use ic_exports::ic_cdk::api::stable::StableMemoryError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("insufficient space available")]
    InsufficientSpace,

    #[error("stable memory error")]
    StableMemory,

    #[error(
        "attempted downgrade, or trying to load a version older than what is currently stored"
    )]
    AttemptedDowngrade,

    #[error("candid error: {0}")]
    Candid(#[from] ic_exports::ic_cdk::export::candid::Error),

    #[error("existing version is newer")]
    ExistingVersionIsNewer,
}

// Required because `StableMemoryError` doesn't implement Debug
impl From<StableMemoryError> for Error {
    fn from(_: StableMemoryError) -> Self {
        Self::StableMemory
    }
}
