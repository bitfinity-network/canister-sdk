use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(CandidType, Debug, Error, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum SchedulerError {
    /// Recoverable error during a scheduler task execution.
    ///
    /// If task execution returns this type of error, the task will be retried according to the
    /// retry policy set for this task.
    #[error("TaskExecutionFailed: {0}")]
    TaskExecutionFailed(String),

    /// Error during task execution that is unlikely to be fixed by retrying the task.
    ///
    /// If a task returns this type of error, the task will not be rescheduled according to the
    /// retry policy and will be considered failed right away.
    #[error("Unrecoverable task error: {0}")]
    Unrecoverable(String),
}

/// Result type for the scheduler
pub type Result<T> = std::result::Result<T, SchedulerError>;
