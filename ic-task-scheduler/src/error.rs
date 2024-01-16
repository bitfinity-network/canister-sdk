use ic_kit::RejectionCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("TaskExecutionFailed: {0}")]
    TaskExecutionFailed(String),
    #[error("storage error: {0}")]
    StorageError(#[from] ic_stable_structures::Error),
    #[error("ic call error")]
    IcCallError(RejectionCode),
}

impl From<RejectionCode> for SchedulerError {
    fn from(code: RejectionCode) -> Self {
        SchedulerError::IcCallError(code)
    }
}
