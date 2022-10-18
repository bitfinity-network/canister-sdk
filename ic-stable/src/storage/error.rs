use ic_exports::stable_structures::{btreemap, cell};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("stable memory can't grow anymore")]
    OutOfStableMemory,
    #[error("value bytes interpretation is too large for stable structure: {0}")]
    ValueTooLarge(u64),
    #[error("memory manager and stable structure has incompatible types")]
    IncompatibleVersions,
}

impl From<cell::InitError> for StorageError {
    fn from(e: cell::InitError) -> Self {
        match e {
            cell::InitError::IncompatibleVersion { .. } => Self::IncompatibleVersions,
            cell::InitError::ValueTooLarge { value_size } => Self::ValueTooLarge(value_size),
        }
    }
}

impl From<cell::ValueError> for StorageError {
    fn from(e: cell::ValueError) -> Self {
        match e {
            cell::ValueError::ValueTooLarge { value_size } => Self::ValueTooLarge(value_size),
        }
    }
}

impl From<btreemap::InsertError> for StorageError {
    fn from(e: btreemap::InsertError) -> Self {
        match e {
            btreemap::InsertError::KeyTooLarge { given, .. } => Self::ValueTooLarge(given as _),
            btreemap::InsertError::ValueTooLarge { given, .. } => Self::ValueTooLarge(given as _),
        }
    }
}
