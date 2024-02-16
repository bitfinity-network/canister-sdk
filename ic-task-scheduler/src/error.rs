use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("TaskExecutionFailed: {0}")]
    TaskExecutionFailed(String),
    #[error("storage error: {0}")]
    StorageError(#[from] ic_stable_structures::Error),
}
