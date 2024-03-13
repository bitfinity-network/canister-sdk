use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulerError {
    #[error("TaskExecutionFailed: {0}")]
    TaskExecutionFailed(String),
}

/// Result type for the scheduler
pub type Result<T> = std::result::Result<T, SchedulerError>;
