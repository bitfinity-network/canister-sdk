use dfinity_stable_structures::{cell, log, vec, GrowFailed};
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
    #[error("the vector type is not compatible with the current vector")]
    IncompatibleElementType,
    #[error("bad magic number: actual: {actual:?}, expected: {expected:?}")]
    BadMagic { actual: [u8; 3], expected: [u8; 3] },
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

// impl From<btreemap::InsertError> for Error {
//     fn from(e: btreemap::InsertError) -> Self {
//         match e {
//             btreemap::InsertError::KeyTooLarge { given, .. } => Self::ValueTooLarge(given as _),
//             btreemap::InsertError::ValueTooLarge { given, .. } => Self::ValueTooLarge(given as _),
//         }
//     }
// }

impl From<log::InitError> for Error {
    fn from(_: log::InitError) -> Self {
        // All `log::InitError` variants is versioning errors.
        Self::IncompatibleVersions
    }
}

impl From<vec::InitError> for Error {
    fn from(e: vec::InitError) -> Self {
        match e {
            vec::InitError::IncompatibleVersion(_) => Self::IncompatibleVersions,
            vec::InitError::IncompatibleElementType => Self::IncompatibleElementType,
            vec::InitError::OutOfMemory => Self::OutOfStableMemory,
            vec::InitError::BadMagic { actual, expected } => Self::BadMagic { actual, expected },
        }
    }
}

impl From<GrowFailed> for Error {
    fn from(_: GrowFailed) -> Self {
        Self::OutOfStableMemory
    }
}
