use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("TaskExecutionFailed: {0}")]
    TaskExecutionFailed(String),
}
