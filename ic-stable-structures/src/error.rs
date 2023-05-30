use ic_exports::stable_structures::{btreemap, cell, log, GrowFailed};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("stable memory can't grow anymore")]
    OutOfStableMemory,
    #[error("value bytes interpretation is too large for stable structure: {0}")]
    ValueTooLarge(u64),
    #[error("memory manager and stable structure has incompatible versions")]
    IncompatibleVersions,
}

impl From<cell::InitError> for Error {
    fn from(e: cell::InitError) -> Self {
        match e {
            cell::InitError::IncompatibleVersion { .. } => Self::IncompatibleVersions,
            cell::InitError::ValueTooLarge { value_size } => Self::ValueTooLarge(value_size),
        }
    }
}

impl From<cell::ValueError> for Error {
    fn from(e: cell::ValueError) -> Self {
        match e {
            cell::ValueError::ValueTooLarge { value_size } => Self::ValueTooLarge(value_size),
        }
    }
}

impl From<btreemap::InsertError> for Error {
    fn from(e: btreemap::InsertError) -> Self {
        match e {
            btreemap::InsertError::KeyTooLarge { given, .. } => Self::ValueTooLarge(given as _),
            btreemap::InsertError::ValueTooLarge { given, .. } => Self::ValueTooLarge(given as _),
        }
    }
}

impl From<log::InitError> for Error {
    fn from(_: log::InitError) -> Self {
        // All `log::InitError` variants is versioning errors.
        Self::IncompatibleVersions
    }
}

impl From<GrowFailed> for Error {
    fn from(_: GrowFailed) -> Self {
        Self::OutOfStableMemory
    }
}
