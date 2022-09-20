pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("candid error: {0}")]
    Candid(#[from] candid::Error),
    #[error("insert error: {0}")]
    Insert(crate::InsertError),
}

impl From<crate::InsertError> for Error {
    fn from(e: crate::InsertError) -> Self {
        Self::Insert(e)
    }
}
