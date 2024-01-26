use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemMapError {
    #[error("file error: {0}")]
    FileOpenError(#[from] std::io::Error),
    #[error("address space limit exceeded")]
    OutOfAddressSpace { claimed: usize, limit: usize },
    #[error("access out of bounds")]
    AccessOutOfBounds,
    #[error("new length should be page size multiple")]
    SizeShouldBePageSizeMultiple,
    #[error("invalid source file name")]
    InvalidSourceFileName,
}

pub type MemMapResult<T> = Result<T, MemMapError>;
