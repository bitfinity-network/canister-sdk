use std::{sync::Arc, pin::Pin, future::Future};

use ic_stable_structures::Bound;
use ic_stable_structures::ChunkSize;
use ic_stable_structures::SlicedStorable;
use ic_stable_structures::Storable;
use ic_stable_structures::UnboundedMapStructure;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use crate::SchedulerError;
use crate::time::time_secs;

/// A sync task is a unit of work that can be executed by the scheduler.
pub trait Task {
    /// Execute the task and return the next task to execute.
    fn execute(&self, task_scheduler: Box<dyn 'static + TaskScheduler<Self>>) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>>;
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
    /// Returns a list of the processed tasks
    pub async fn run(&self) -> Result<(), SchedulerError> {
        
        let mut to_be_reprocessed = Vec::new();
        {
            let mut lock = self.pending_tasks.lock();
            while let Some(key) = lock.first_key() {
                let task = lock.remove(&key);
                drop(lock);
                if let Some(task) = task {
                    let now_timestamp_secs = time_secs();

                    if task.options.execute_after_timestamp_in_secs > now_timestamp_secs {
                        to_be_reprocessed.push(task);
                    } else {
                        let task_scheduler = self.clone();
                        ic_kit::ic::spawn(async move {
                            match task.task.execute(Box::new(task_scheduler.clone())).await {
                                Ok(()) => {
//                                    processed_tasks.push(task);
                                },
                                Err(_) => {
                                    if task.options.max_retries > 0 {
                                        let mut task = task;
                                        task.options.max_retries = task.options.max_retries - 1;
                                        task.options.execute_after_timestamp_in_secs = now_timestamp_secs + task.options.retry_delay_secs;
                                        task_scheduler.append_task(task)
                                    }
                                },
                            }
                        });
                    }
                }
                lock = self.pending_tasks.lock();
            }
        }
        self.append_tasks(to_be_reprocessed);
        Ok(())
    }
}

pub trait SchedulerExecutor {
    fn execute(&self);
}

pub trait TaskScheduler<T: 'static + Task> {
    fn append_task(&self, task: ScheduledTask<T>);
    fn append_tasks(&self, tasks: Vec<ScheduledTask<T>>);
}

impl <T: 'static + Task, P: 'static + UnboundedMapStructure<u32, ScheduledTask<T>>> Clone for Scheduler<T, P> {
    fn clone(&self) -> Self {
        Self { pending_tasks: self.pending_tasks.clone(), phantom: self.phantom.clone() }
    }
}

impl <T: 'static + Task, P: 'static + UnboundedMapStructure<u32, ScheduledTask<T>>> TaskScheduler<T> for Scheduler<T, P> {
    fn append_task(&self, task: ScheduledTask<T>) {
        let mut lock = self.pending_tasks.lock();
        let key = lock.last_key().map(|val| val + 1).unwrap_or_default();
        lock.insert(&key, &task);
    }

    fn append_tasks(&self, tasks: Vec<ScheduledTask<T>>) {
        if tasks.is_empty() {
            return;
        };

        let mut lock = self.pending_tasks.lock();
        let mut key = lock.last_key().map(|val| val + 1).unwrap_or_default();

        for task in tasks {
            lock.insert(&key, &task);
            key = key + 1;
        }
    }
}

/// Scheduling options for a task
#[derive(Serialize, Deserialize)]
pub struct TaskOptions {
    max_retries: u16,
    retry_delay_secs: u64,
    execute_after_timestamp_in_secs: u64,
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
    pub fn with_execute_after_timestamp_in_secs(mut self, execute_after_timestamp_in_secs: u64) -> Self {
        self.execute_after_timestamp_in_secs = execute_after_timestamp_in_secs;
        self
    }
}

#[cfg(test)] 
mod test {

    use ic_kit::MockContext;
    use ic_stable_structures::{StableUnboundedMap, VectorMemory};

    use super::*;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub enum TestTask {
        StepOne,
        StepTwo,
        StepThree,
    }

    impl Task for TestTask {
        fn execute(&self, task_scheduler: Box<dyn 'static + TaskScheduler<Self>>) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
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

    #[tokio::test]
    async fn test_run_scheduler() {
        MockContext::new().inject();
        let map = StableUnboundedMap::new(VectorMemory::default());
        let scheduler = Scheduler::new(map);
        scheduler.append_task(TestTask::StepOne.into());
        scheduler.run().await.unwrap();
    }

    #[tokio::test]
    async fn test_task_option_execute_after_timestamp() {
        let map = StableUnboundedMap::new(VectorMemory::default());
        let scheduler = Scheduler::new(map);

        scheduler.append_task((TestTask::StepOne, TaskOptions::new()
            .with_execute_after_timestamp_in_secs(time_secs() + 2)
        ).into());

        scheduler.run().await.unwrap();

        todo!()
    }

    #[tokio::test]
    async fn test_task_failure_and_retry() {
        let map = StableUnboundedMap::new(VectorMemory::default());
        let scheduler = Scheduler::new(map);
        
        scheduler.append_task((TestTask::StepOne, TaskOptions::new()
            .with_max_retries(3)
        ).into());
        
        scheduler.run().await.unwrap();

        todo!()
    }

    #[tokio::test]
    async fn test_task_retry_delay() {
        let map = StableUnboundedMap::new(VectorMemory::default());
        let scheduler = Scheduler::new(map);
        
        scheduler.append_task((TestTask::StepOne, TaskOptions::new()
            .with_max_retries(5)
            .with_retry_delay_secs(2)
        ).into());
        
        scheduler.run().await.unwrap();

        todo!()
    }

}