use std::{sync::Arc, pin::Pin, future::Future};

use dfinity_stable_structures::Storable;
use dfinity_stable_structures::storable::Bound;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use crate::{Result, SlicedStorable, UnboundedMapStructure, ChunkSize};

/// A sync task is a unit of work that can be executed by the scheduler.
pub trait Task {
    /// Execute the task and return the next task to execute.
    fn execute(&self, task_scheduler: Box<dyn 'static + TaskScheduler<Self>>) -> Pin<Box<dyn Future<Output = Result<()>>>>;
}

/// A scheduled task is a task that is ready to be executed.
#[derive(Default, Serialize, Deserialize)]
pub struct ScheduledTask<T: Task> {
    task: T,
    options: TaskOptions,
}

impl <T: Task> ScheduledTask<T> {

    pub fn new(task: T) -> Self {
        Self {
            task,
            options: Default::default(),
        }
    }

    pub fn with_options(task: T, options: TaskOptions) -> Self {
        Self {
            task,
            options,
        }
    }

}

impl <T: Task> From<T> for ScheduledTask<T> {
    fn from(task: T) -> Self {
        Self::new(task)
    }
}

impl <T: Task> From<(T, TaskOptions)> for ScheduledTask<T> {
    fn from((task, options): (T, TaskOptions)) -> Self {
        Self::with_options(task, options)
    }
}

impl <T: 'static + Task + Serialize + DeserializeOwned> Storable for ScheduledTask<T> {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        bincode::serialize(self).expect("failed to serialize ScheduledTask").into()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        bincode::deserialize(&bytes).expect("failed to deserialize ScheduledTask")
    }

    const BOUND: Bound = Bound::Unbounded;
}

impl <T: 'static + Task + Serialize + DeserializeOwned> SlicedStorable for ScheduledTask<T> {
    const CHUNK_SIZE: ChunkSize = 128;
}

/// A scheduler is responsible for executing tasks.
#[derive(Clone)]
pub struct Scheduler<T: 'static + Task, P: 'static + UnboundedMapStructure<u32, ScheduledTask<T>>> {
    pending_tasks: Arc<Mutex<P>>,
    phantom: std::marker::PhantomData<T>,
}

impl <T: 'static + Task, P: 'static + UnboundedMapStructure<u32, ScheduledTask<T>>> Scheduler<T, P> {

    pub fn new(pending_tasks: P) -> Self {
        Self {
            pending_tasks: Arc::new(Mutex::new(pending_tasks)),
            phantom: std::marker::PhantomData,
        }
    }

    /// Execute all pending tasks.
    pub async fn run(&self) -> Result<()> {
        let mut lock = self.pending_tasks.lock();

        while let Some(key) = lock.first_key() {
            if let Some(task) = lock.remove(&key) {
                drop(lock);
                let task_scheduler = Box::new(Self {
                    pending_tasks: self.pending_tasks.clone(),
                    phantom: std::marker::PhantomData,
                });
                task.task.execute(task_scheduler).await?;
            }
            lock = self.pending_tasks.lock();
        }
        Ok(())
    }
}

pub trait SchedulerExecutor {
    fn execute(&self);
}

pub trait TaskScheduler<T: 'static + Task> {
    fn append_task(&self, task: ScheduledTask<T>);
}

impl <T: 'static + Task, P: 'static + UnboundedMapStructure<u32, ScheduledTask<T>>> TaskScheduler<T> for Scheduler<T, P> {
    fn append_task(&self, task: ScheduledTask<T>) {
        let mut lock = self.pending_tasks.lock();
        let key = lock.last_key().map(|val| val + 1).unwrap_or_default();
        lock.insert(&key, &task);
    }
}

/// Scheduling options for a task
#[derive(Default, Serialize, Deserialize)]
pub struct TaskOptions {
    max_retries: u32,
    retry_delay_secs: u32,
    execute_after_timestamp_in_secs: u32,
}

impl TaskOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of retries for a failed task. Default is 0.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the delay between retries for a failed task. Default is 0.
    pub fn with_retry_delay_secs(mut self, retry_delay_secs: u32) -> Self {
        self.retry_delay_secs = retry_delay_secs;
        self
    }

    /// Set the timestamp after which the task can be executed. Default is 0.
    pub fn with_execute_after_timestamp_in_secs(mut self, execute_after_timestamp_in_secs: u32) -> Self {
        self.execute_after_timestamp_in_secs = execute_after_timestamp_in_secs;
        self
    }
}

#[cfg(test)] 
mod test {

    use dfinity_stable_structures::{Storable, VectorMemory};
    use ic_exports::ic_kit::MockContext;

    use crate::StableUnboundedMap;
    use super::*;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
    pub enum TestTask {
        StepOne,
        StepTwo,
        StepThree,
    }

    impl Task for TestTask {
        fn execute(&self, task_scheduler: Box<dyn 'static + TaskScheduler<Self>>) -> Pin<Box<dyn Future<Output = Result<()>>>> {
            match self {
                TestTask::StepOne => Box::pin(async move {
                    println!("StepOne");
                    // Append the next task to be executed
                    task_scheduler.append_task(TestTask::StepTwo.into());
                    Ok(())
                }),
                TestTask::StepTwo => Box::pin(async move {
                    println!("StepTwo");

                    // More tasks can be appended to the scheduler. BEWARE of circular dependencies!!
                    task_scheduler.append_task(TestTask::StepThree.into());
                    task_scheduler.append_task(TestTask::StepThree.into());
                    Ok(())
                }),
                TestTask::StepThree => Box::pin(async move {
                    println!("StepThree");
                    // the last task does not append anything to the scheduler
                    Ok(())
                }),
            }
        }
    }

    impl SlicedStorable for TestTask {
        const CHUNK_SIZE: u16 = 128;
    }

    impl Storable for TestTask {
        fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
            serde_json::to_vec(self).unwrap().into()
        }

        fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
            serde_json::from_slice(bytes.as_ref()).unwrap()
        }

        const BOUND: dfinity_stable_structures::storable::Bound = dfinity_stable_structures::storable::Bound::Unbounded;
    }

    #[tokio::test]
    async fn test_spawn() {
        MockContext::new().inject();
        let map = StableUnboundedMap::new(VectorMemory::default());
        let scheduler = Scheduler::new(map);
        
        scheduler.append_task(TestTask::StepOne.into());
        scheduler.run().await.unwrap();
    }

}