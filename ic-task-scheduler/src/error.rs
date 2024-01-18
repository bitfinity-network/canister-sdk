use ic_kit::RejectionCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("TaskExecutionFailed: {0}")]
    TaskExecutionFailed(String),
    #[error("storage error: {0}")]
    StorageError(#[from] ic_stable_structures::Error),
    #[error("ic call error: {1}")]
    IcCallError(RejectionCode, String),
}

impl From<(RejectionCode, String)> for SchedulerError {
    fn from((code, message): (RejectionCode, String)) -> Self {
        SchedulerError::IcCallError(code, message)
    }
}
