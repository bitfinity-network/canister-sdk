use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemMapError {
    #[error("file error: {0}")]
    FileOpenError(#[from] std::io::Error),
    #[error("address space limit exceeded")]
    OutOfAddressSpace{claimed: u64, limit: u64},
    #[error("access out of bounds")]
    AccessOutOfBounds,
}

pub type MemMapResult<T> = Result<T, MemMapError>;