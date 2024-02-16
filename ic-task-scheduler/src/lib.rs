mod error;
pub mod retry;
pub mod scheduler;
pub mod task;
mod time;

pub use error::SchedulerError;
/// Result type for the scheduler
pub type Result<T> = std::result::Result<T, SchedulerError>;
