use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(CandidType, Debug, Error, PartialEq, Eq, Serialize, Deserialize,Clone)]
pub enum SchedulerError {
    #[error("TaskExecutionFailed: {0}")]
    TaskExecutionFailed(String),
}

/// Result type for the scheduler
pub type Result<T> = std::result::Result<T, SchedulerError>;
