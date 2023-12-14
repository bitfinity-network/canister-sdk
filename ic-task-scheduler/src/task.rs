use std::future::Future;
use std::pin::Pin;

use ic_stable_structures::{Bound, ChunkSize, SlicedStorable, Storable};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::retry::{BackoffPolicy, RetryPolicy, RetryStrategy};
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
#[derive(Default, Serialize, Deserialize, PartialEq, Eq, Debug)]
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
#[derive(Default, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct TaskOptions {
    pub(crate) failures: u32,
    pub(crate) execute_after_timestamp_in_secs: u64,
    pub(crate) retry_strategy: RetryStrategy,
}

impl TaskOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the retry policy for a failed task to RetryPolicy::MaxRetries.
    pub fn with_max_retries_policy(mut self, retries: u32) -> Self {
        self.retry_strategy.retry_policy = RetryPolicy::MaxRetries { retries };
        self
    }

    /// Set the retry policy for a failed task. Default is RetryPolicy::None.
    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_strategy.retry_policy = retry_policy;
        self
    }

    /// Set the backoff policy for a failed task to BackoffPolicy::Fixed.
    pub fn with_fixed_backoff_policy(mut self, secs: u32) -> Self {
        self.retry_strategy.backoff_policy = BackoffPolicy::Fixed { secs };
        self
    }

    /// Set the backoff policy for a failed task. Default is BackoffPolicy::Fixed{ secs: 2 }.
    pub fn with_backoff_policy(mut self, backoff_policy: BackoffPolicy) -> Self {
        self.retry_strategy.backoff_policy = backoff_policy;
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

#[cfg(test)]
mod test {

    use super::*;

    #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
    struct TestTask {}

    impl Task for TestTask {
        fn execute(
            &self,
            _task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
        ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
            todo!()
        }
    }

    #[test]
    fn test_storable_task() {
        {
            let task = ScheduledTask::with_options(
                TestTask {},
                TaskOptions::new()
                    .with_max_retries_policy(3)
                    .with_fixed_backoff_policy(2),
            );

            let serialized = task.to_bytes();
            let deserialized = ScheduledTask::<TestTask>::from_bytes(serialized);

            assert_eq!(task, deserialized);
        }

        {
            let task = ScheduledTask::with_options(
                TestTask {},
                TaskOptions::new()
                    .with_retry_policy(RetryPolicy::None)
                    .with_backoff_policy(BackoffPolicy::None),
            );

            let serialized = task.to_bytes();
            let deserialized = ScheduledTask::<TestTask>::from_bytes(serialized);

            assert_eq!(task, deserialized);
        }

        {
            let task = ScheduledTask::with_options(
                TestTask {},
                TaskOptions::new()
                    .with_retry_policy(RetryPolicy::None)
                    .with_backoff_policy(BackoffPolicy::Exponential {
                        secs: 2,
                        multiplier: 2,
                    }),
            );

            let serialized = task.to_bytes();
            let deserialized = ScheduledTask::<TestTask>::from_bytes(serialized);

            assert_eq!(task, deserialized);
        }

        {
            let task = ScheduledTask::with_options(
                TestTask {},
                TaskOptions::new()
                    .with_retry_policy(RetryPolicy::Infinite)
                    .with_backoff_policy(BackoffPolicy::Variable {
                        secs: vec![12, 56, 76],
                    }),
            );

            let serialized = task.to_bytes();
            let deserialized = ScheduledTask::<TestTask>::from_bytes(serialized);

            assert_eq!(task, deserialized);
        }
    }
}
