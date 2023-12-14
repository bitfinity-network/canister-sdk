use std::future::Future;
use std::pin::Pin;

use ic_stable_structures::{Bound, ChunkSize, SlicedStorable, Storable};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::scheduler::TaskScheduler;
use crate::SchedulerError;

/// A sync task is a unit of work that can be executed by the scheduler.
pub trait Task {
    /// Execute the task and return the next task to execute.
    fn execute(
        &self,
        task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>>;
}

/// A scheduled task is a task that is ready to be executed.
#[derive(Default, Serialize, Deserialize)]
pub struct ScheduledTask<T: Task> {
    pub(crate) task: T,
    pub(crate) options: TaskOptions,
}

impl<T: Task> ScheduledTask<T> {
    pub fn new(task: T) -> Self {
        Self {
            task,
            options: Default::default(),
        }
    }

    pub fn with_options(task: T, options: TaskOptions) -> Self {
        Self { task, options }
    }
}

impl<T: Task> From<T> for ScheduledTask<T> {
    fn from(task: T) -> Self {
        Self::new(task)
    }
}

impl<T: Task> From<(T, TaskOptions)> for ScheduledTask<T> {
    fn from((task, options): (T, TaskOptions)) -> Self {
        Self::with_options(task, options)
    }
}

impl<T: 'static + Task + Serialize + DeserializeOwned> Storable for ScheduledTask<T> {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        bincode::serialize(self)
            .expect("failed to serialize ScheduledTask")
            .into()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        bincode::deserialize(&bytes).expect("failed to deserialize ScheduledTask")
    }

    const BOUND: Bound = Bound::Unbounded;
}

impl<T: 'static + Task + Serialize + DeserializeOwned> SlicedStorable for ScheduledTask<T> {
    const CHUNK_SIZE: ChunkSize = 128;
}

/// Scheduling options for a task
#[derive(Serialize, Deserialize)]
pub struct TaskOptions {
    pub(crate) max_retries: u16,
    pub(crate) retry_delay_secs: u64,
    pub(crate) execute_after_timestamp_in_secs: u64,
}

impl Default for TaskOptions {
    fn default() -> Self {
        Self {
            max_retries: 0,
            retry_delay_secs: 2,
            execute_after_timestamp_in_secs: 0,
        }
    }
}

impl TaskOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of retries for a failed task. Default is 0.
    pub fn with_max_retries(mut self, max_retries: u16) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the delay between retries for a failed task. Default is 2.
    pub fn with_retry_delay_secs(mut self, retry_delay_secs: u64) -> Self {
        self.retry_delay_secs = retry_delay_secs;
        self
    }

    /// Set the timestamp after which the task can be executed. Default is 0.
    pub fn with_execute_after_timestamp_in_secs(
        mut self,
        execute_after_timestamp_in_secs: u64,
    ) -> Self {
        self.execute_after_timestamp_in_secs = execute_after_timestamp_in_secs;
        self
    }
}
