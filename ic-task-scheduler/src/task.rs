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
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
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

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct InnerScheduledTask<T: Task> {
    pub(crate) id: u32,
    pub(crate) task: T,
    pub(crate) options: TaskOptions,
    pub(crate) status: TaskStatus,
}

impl<T: Task> InnerScheduledTask<T> {
    /// Creates a new InnerScheduledTask with the given status
    pub fn with_status(id: u32, task: ScheduledTask<T>, status: TaskStatus) -> Self {
        Self {
            id,
            task: task.task,
            options: task.options,
            status,
        }
    }

    /// Creates a new InnerScheduledTask with Waiting status
    pub fn waiting(id: u32, task: ScheduledTask<T>, timestamp_secs: u64) -> Self {
        Self {
            id,
            task: task.task,
            options: task.options,
            status: TaskStatus::Waiting { timestamp_secs },
        }
    }

    /// Creates a new InnerScheduledTask with Complete status
    pub fn completed(id: u32, task: ScheduledTask<T>, timestamp_secs: u64) -> Self {
        Self {
            id,
            task: task.task,
            options: task.options,
            status: TaskStatus::Completed { timestamp_secs },
        }
    }

    /// Creates a new InnerScheduledTask with TimeoutOrPanic status
    pub fn timeout_or_panic(id: u32, task: ScheduledTask<T>, timestamp_secs: u64) -> Self {
        Self {
            id,
            task: task.task,
            options: task.options,
            status: TaskStatus::TimeoutOrPanic { timestamp_secs },
        }
    }

    /// Creates a new InnerScheduledTask with Running status
    pub fn running(id: u32, task: ScheduledTask<T>, timestamp_secs: u64) -> Self {
        Self {
            id,
            task: task.task,
            options: task.options,
            status: TaskStatus::Running { timestamp_secs },
        }
    }

    /// Returs the status of the task
    pub fn status(&self) -> &TaskStatus {
        &self.status
    }

    /// Returs the options of the task
    pub fn options(&self) -> &TaskOptions {
        &self.options
    }

    /// Returs the task
    pub fn task(&self) -> &T {
        &self.task
    }

    /// Returs the task id
    pub fn id(&self) -> u32 {
        self.id
    }
}

impl<T: 'static + Task + Serialize + DeserializeOwned> Storable for InnerScheduledTask<T> {
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

impl<T: 'static + Task + Serialize + DeserializeOwned> SlicedStorable for InnerScheduledTask<T> {
    const CHUNK_SIZE: ChunkSize = 128;
}

/// The status of a task in the scheduler
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum TaskStatus {
    /// The task is waiting to be executed
    Waiting { timestamp_secs: u64 },
    /// The task execution completed successfully
    Completed { timestamp_secs: u64 },
    /// The task is running
    Running { timestamp_secs: u64 },
    /// The task execution failed and no more retries are allowed
    Failed {
        timestamp_secs: u64,
        error: SchedulerError,
    },
    /// The task has been running for long time. It could be stuck or panicking
    TimeoutOrPanic { timestamp_secs: u64 },
}

impl TaskStatus {
    /// Creates a new TaskStatus::Waiting with the given timestamp in seconds
    pub fn waiting(timestamp_secs: u64) -> Self {
        Self::Waiting { timestamp_secs }
    }

    /// Creates a new TaskStatus::Completed with the given timestamp in seconds
    pub fn completed(timestamp_secs: u64) -> Self {
        Self::Completed { timestamp_secs }
    }

    /// Creates a new TaskStatus::Failed with the given timestamp in seconds and error
    pub fn failed(timestamp_secs: u64, error: SchedulerError) -> Self {
        Self::Failed {
            timestamp_secs,
            error,
        }
    }

    /// Creates a new TaskStatus::Running with the given timestamp in seconds
    pub fn running(timestamp_secs: u64) -> Self {
        Self::Running { timestamp_secs }
    }

    /// Creates a new TaskStatus::TimeoutOrPanic with the given timestamp in seconds
    pub fn timeout_or_panic(timestamp_secs: u64) -> Self {
        Self::TimeoutOrPanic { timestamp_secs }
    }

    /// Returns the timestamp of the status
    pub fn timestamp_secs(&self) -> u64 {
        match self {
            TaskStatus::Waiting { timestamp_secs } => *timestamp_secs,
            TaskStatus::Completed { timestamp_secs } => *timestamp_secs,
            TaskStatus::Running { timestamp_secs } => *timestamp_secs,
            TaskStatus::TimeoutOrPanic { timestamp_secs } => *timestamp_secs,
            TaskStatus::Failed { timestamp_secs, .. } => *timestamp_secs,
        }
    }
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
            let task = InnerScheduledTask {
                id: 0,
                task: TestTask {},
                options: TaskOptions::new()
                    .with_max_retries_policy(3)
                    .with_fixed_backoff_policy(2),
                status: TaskStatus::Waiting { timestamp_secs: 0 },
            };

            let serialized = task.to_bytes();
            let deserialized = InnerScheduledTask::<TestTask>::from_bytes(serialized);

            assert_eq!(task, deserialized);
        }

        {
            let task = InnerScheduledTask {
                id: 0,
                task: TestTask {},
                options: TaskOptions::new()
                    .with_retry_policy(RetryPolicy::None)
                    .with_backoff_policy(BackoffPolicy::None),
                status: TaskStatus::Waiting { timestamp_secs: 0 },
            };

            let serialized = task.to_bytes();
            let deserialized = InnerScheduledTask::<TestTask>::from_bytes(serialized);

            assert_eq!(task, deserialized);
        }

        {
            let task = InnerScheduledTask {
                id: 0,
                task: TestTask {},
                options: TaskOptions::new()
                    .with_retry_policy(RetryPolicy::None)
                    .with_backoff_policy(BackoffPolicy::Exponential {
                        secs: 2,
                        multiplier: 2,
                    }),
                status: TaskStatus::Completed {
                    timestamp_secs: 1230,
                },
            };

            let serialized = task.to_bytes();
            let deserialized = InnerScheduledTask::<TestTask>::from_bytes(serialized);

            assert_eq!(task, deserialized);
        }

        {
            let task = InnerScheduledTask {
                id: 0,
                task: TestTask {},
                options: TaskOptions::new()
                    .with_retry_policy(RetryPolicy::Infinite)
                    .with_backoff_policy(BackoffPolicy::Variable {
                        secs: vec![12, 56, 76],
                    }),
                status: TaskStatus::Running {
                    timestamp_secs: 21230,
                },
            };

            let serialized = task.to_bytes();
            let deserialized = InnerScheduledTask::<TestTask>::from_bytes(serialized);

            assert_eq!(task, deserialized);
        }
    }
}
