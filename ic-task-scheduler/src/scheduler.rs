use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use ic_kit::RejectionCode;
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{MemoryManager, StableVec, UnboundedMapStructure, VecStructure};
use parking_lot::Mutex;

use crate::task::{ScheduledTask, Task};
use crate::time::time_secs;
use crate::{Result, SchedulerError};

type SchedulerErrorCallback<T> = Box<dyn 'static + Fn(ScheduledTask<T>, SchedulerError) + Send>;
//type SaveStateQueryCallback =
//    Box<dyn 'static + Fn(dyn Future<Output = std::result::Result<(), RejectionCode>>) + Send>;

type SaveStateQueryCallback = Box<
    dyn Fn() -> Pin<Box<dyn Future<Output = std::result::Result<(), RejectionCode>>>> + Send + Sync,
>;

/// A scheduler is responsible for executing tasks.
pub struct Scheduler<T, P, M>
where
    T: 'static + Task,
    P: 'static + UnboundedMapStructure<u32, ScheduledTask<T>>,
    M: 'static + Memory,
{
    pending_tasks: Arc<Mutex<P>>,
    phantom: std::marker::PhantomData<T>,
    /// Queue containing the tasks to be executed in a loop
    tasks_queue: Arc<RefCell<StableVec<u32, M>>>,
    /// Callback to be called when a task fails.
    failed_task_callback: Arc<Option<SchedulerErrorCallback<T>>>,
    /// Callback to be called to save the current canister state to prevent panicking tasks.
    save_state_query_callback: Arc<Option<SaveStateQueryCallback>>,
}

impl<T, P, M> Scheduler<T, P, M>
where
    T: 'static + Task,
    P: 'static + UnboundedMapStructure<u32, ScheduledTask<T>>,
    M: 'static + Memory,
{
    /// Create a new scheduler.
    pub fn new(
        pending_tasks: P,
        memory_manager: &dyn MemoryManager<M, u8>,
        memory_id: u8,
    ) -> Result<Self> {
        Ok(Self {
            pending_tasks: Arc::new(Mutex::new(pending_tasks)),
            phantom: std::marker::PhantomData,
            tasks_queue: Arc::new(RefCell::new(StableVec::new(memory_manager.get(memory_id))?)),
            failed_task_callback: Arc::new(None),
            save_state_query_callback: Arc::new(None),
        })
    }

    /// Set a callback to be called when a task fails.
    pub fn set_failed_task_callback<F: 'static + Send + Fn(ScheduledTask<T>, SchedulerError)>(
        &mut self,
        cb: F,
    ) {
        self.failed_task_callback = Arc::new(Some(Box::new(cb)));
    }

    /// Set a callback to be called to save the current canister state to prevent panicking tasks.
    pub fn set_save_state_query_callback(&mut self, cb: SaveStateQueryCallback) {
        self.save_state_query_callback = Arc::new(Some(Box::new(cb)));
    }

    /// Execute all pending tasks.
    /// Each task is executed asynchronously in a dedicated ic_cdk::spawn call.
    /// This function does not wait for the tasks to complete.
    /// Returns the number of tasks that have been launched.
    pub async fn run(&self) -> Result<u32> {
        self.run_with_timestamp(time_secs()).await
    }

    async fn run_with_timestamp(&self, now_timestamp_secs: u64) -> Result<u32> {
        let mut to_be_reprocessed = Vec::new();
        let mut task_execution_started = 0;
        {
            let mut lock = self.pending_tasks.lock();
            while let Some(key) = lock.first_key() {
                let task = lock.remove(&key);
                drop(lock);
                if let Some(task) = task {
                    if task.options.execute_after_timestamp_in_secs > now_timestamp_secs {
                        to_be_reprocessed.push(task);
                    } else {
                        task_execution_started += 1;
                        let task_scheduler = self.clone();
                        Self::spawn(async move {
                            if let Err(err) =
                                task.task.execute(Box::new(task_scheduler.clone())).await
                            {
                                let mut task = task;
                                task.options.failures += 1;
                                let (should_retry, retry_delay) = task
                                    .options
                                    .retry_strategy
                                    .should_retry(task.options.failures);
                                if should_retry {
                                    task.options.execute_after_timestamp_in_secs =
                                        now_timestamp_secs + (retry_delay as u64);
                                    task_scheduler.append_task(task)
                                } else if let Some(cb) = &*task_scheduler.failed_task_callback {
                                    cb(task, err);
                                }
                            }
                        });
                    }
                }
                lock = self.pending_tasks.lock();
            }
        }
        self.append_tasks(to_be_reprocessed);
        Ok(task_execution_started)
    }

    // We use tokio for testing instead of ic_kit::ic::spawn because the latter blocks the current thread
    // waiting for the spawned futures to complete.
    // This makes impossible to test concurrent behavior.
    #[cfg(test)]
    fn spawn<F: 'static + std::future::Future<Output = ()>>(future: F) {
        tokio::task::spawn_local(future);
    }

    #[cfg(not(test))]
    #[inline(always)]
    fn spawn<F: 'static + std::future::Future<Output = ()>>(future: F) {
        ic_kit::ic::spawn(future);
    }

    /// Save the current state of the scheduler.
    async fn save_state(&self) -> Result<()> {
        if let Some(cb) = &*self.save_state_query_callback {
            cb().await?;
        }
        Ok(())
    }

    /// Remove all the tasks in `pending_tasks` which are contained in `tasks_queue`.
    /// The also clear the values in `tasks_queue`
    fn delete_pending_tasks(&self) -> Result<()> {
        // delete the tasks in the pending tasks
        for task in self.tasks_queue.borrow().iter() {
            self.pending_tasks.lock().remove(&task);
        }
        // empty the task queue
        self.tasks_queue.borrow_mut().clear()?;

        Ok(())
    }
}

pub trait TaskScheduler<T: 'static + Task> {
    fn append_task(&self, task: ScheduledTask<T>);
    fn append_tasks(&self, tasks: Vec<ScheduledTask<T>>);
}

impl<T, P, M> Clone for Scheduler<T, P, M>
where
    T: 'static + Task,
    P: 'static + UnboundedMapStructure<u32, ScheduledTask<T>>,
    M: Memory,
{
    fn clone(&self) -> Self {
        Self {
            pending_tasks: self.pending_tasks.clone(),
            phantom: self.phantom,
            failed_task_callback: self.failed_task_callback.clone(),
            save_state_query_callback: self.save_state_query_callback.clone(),
            tasks_queue: self.tasks_queue.clone(),
        }
    }
}

impl<T, P, M> TaskScheduler<T> for Scheduler<T, P, M>
where
    T: 'static + Task,
    P: 'static + UnboundedMapStructure<u32, ScheduledTask<T>>,
    M: Memory,
{
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
            key += 1;
        }
    }
}

#[cfg(test)]
mod test {

    use ic_stable_structures::stable_structures::DefaultMemoryImpl;
    use ic_stable_structures::{default_ic_memory_manager, VirtualMemory};

    use super::*;

    mod test_execution {

        use std::collections::HashMap;
        use std::future::Future;
        use std::pin::Pin;
        use std::sync::atomic::AtomicBool;
        use std::time::Duration;

        use ic_stable_structures::{StableUnboundedMap, VectorMemory};
        use rand::random;
        use serde::{Deserialize, Serialize};

        use super::*;

        thread_local! {
            pub static STATE: Mutex<HashMap<u32, Vec<String>>> = Mutex::new(HashMap::new())
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub enum SimpleTaskSteps {
            One { id: u32 },
            Two { id: u32 },
            Three { id: u32 },
        }

        impl Task for SimpleTaskSteps {
            fn execute(
                &self,
                task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
            ) -> Pin<Box<dyn Future<Output = Result<()>>>> {
                match self {
                    SimpleTaskSteps::One { id } => {
                        let id = *id;
                        Box::pin(async move {
                            let msg = format!("{} - StepOne", id);
                            println!("{}", msg);
                            STATE.with(|state| {
                                let mut state = state.lock();
                                let entry = state.entry(id).or_default();
                                entry.push(msg);
                            });
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            // Append the next task to be executed
                            task_scheduler.append_task(SimpleTaskSteps::Two { id }.into());
                            Ok(())
                        })
                    }
                    SimpleTaskSteps::Two { id } => {
                        let id = *id;
                        Box::pin(async move {
                            let msg = format!("{} - StepTwo", id);
                            println!("{}", msg);
                            STATE.with(|state| {
                                let mut state = state.lock();
                                let entry = state.entry(id).or_default();
                                entry.push(msg);
                            });
                            // More tasks can be appended to the scheduler. BEWARE of circular dependencies!!
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            task_scheduler.append_task(SimpleTaskSteps::Three { id }.into());
                            task_scheduler.append_task(SimpleTaskSteps::Three { id }.into());
                            Ok(())
                        })
                    }
                    SimpleTaskSteps::Three { id } => {
                        let id = *id;
                        Box::pin(async move {
                            let msg = format!("{} - Done", id);
                            println!("{}", msg);
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            STATE.with(|state| {
                                let mut state = state.lock();
                                let entry = state.entry(id).or_default();
                                entry.push(msg);
                            });
                            // the last task does not append anything to the scheduler
                            Ok(())
                        })
                    }
                }
            }
        }

        #[tokio::test]
        async fn test_run_scheduler() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let scheduler = scheduler();
                    let id = random();
                    scheduler.append_task(SimpleTaskSteps::One { id }.into());

                    let mut completed = false;

                    while !completed {
                        scheduler.run().await.unwrap();
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        STATE.with(|state| {
                            let state = state.lock();
                            let messages = state.get(&id).cloned().unwrap_or_default();
                            if messages.len() == 4 {
                                completed = true;
                                assert_eq!(
                                    messages,
                                    vec![
                                        format!("{} - StepOne", id),
                                        format!("{} - StepTwo", id),
                                        format!("{} - Done", id),
                                        format!("{} - Done", id),
                                    ]
                                );
                            }
                        });
                    }
                })
                .await
        }

        #[tokio::test]
        async fn test_error_cb_not_called_in_case_of_success() {
            let local = tokio::task::LocalSet::new();
            let called = Arc::new(AtomicBool::new(false));
            let called_t = called.clone();
            local
                .run_until(async move {
                    let mut scheduler = scheduler();
                    scheduler.set_failed_task_callback(move |_, _| {
                        called_t.store(true, std::sync::atomic::Ordering::SeqCst);
                    });
                    let id = random();
                    scheduler.append_task(SimpleTaskSteps::One { id }.into());

                    let mut completed = false;

                    while !completed {
                        scheduler.run().await.unwrap();
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        STATE.with(|state| {
                            let state = state.lock();
                            let messages = state.get(&id).cloned().unwrap_or_default();
                            if messages.len() == 4 {
                                completed = true;
                                assert_eq!(
                                    messages,
                                    vec![
                                        format!("{} - StepOne", id),
                                        format!("{} - StepTwo", id),
                                        format!("{} - Done", id),
                                        format!("{} - Done", id),
                                    ]
                                );
                            }
                        });
                    }
                })
                .await;

            assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
        }

        fn scheduler() -> Scheduler<
            SimpleTaskSteps,
            StableUnboundedMap<
                u32,
                ScheduledTask<SimpleTaskSteps>,
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            >,
            VirtualMemory<DefaultMemoryImpl>,
        > {
            let map: StableUnboundedMap<
                u32,
                ScheduledTask<SimpleTaskSteps>,
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            > = StableUnboundedMap::new(VectorMemory::default());
            let memory_manager: ic_stable_structures::IcMemoryManager<
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            > = default_ic_memory_manager();
            Scheduler::new(map, &memory_manager, 1).unwrap()
        }
    }

    mod test_delay {

        use std::collections::HashMap;
        use std::future::Future;
        use std::pin::Pin;
        use std::time::Duration;

        use ic_stable_structures::{StableUnboundedMap, VectorMemory};
        use rand::random;
        use serde::{Deserialize, Serialize};

        use super::*;
        use crate::task::TaskOptions;

        thread_local! {
            pub static STATE: Mutex<HashMap<u32, Vec<String>>> = Mutex::new(HashMap::new())
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub enum SimpleTask {
            StepOne { id: u32 },
        }

        impl Task for SimpleTask {
            fn execute(
                &self,
                _task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
            ) -> Pin<Box<dyn Future<Output = Result<()>>>> {
                match self {
                    SimpleTask::StepOne { id } => {
                        let id = *id;
                        Box::pin(async move {
                            let msg = format!("{} - StepOne", id);
                            println!("{}", msg);
                            STATE.with(|state| {
                                let mut state = state.lock();
                                let entry = state.entry(id).or_default();
                                entry.push(msg);
                            });
                            Ok(())
                        })
                    }
                }
            }
        }

        #[tokio::test]
        async fn test_execute_after_timestamp() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let scheduler = scheduler();
                    let id = random();
                    let timestamp: u64 = random();

                    scheduler.append_task(
                        (
                            SimpleTask::StepOne { id },
                            TaskOptions::new().with_execute_after_timestamp_in_secs(timestamp + 10),
                        )
                            .into(),
                    );

                    for i in 0..10 {
                        // Should not run the task because the execution timestamp is in the future
                        scheduler.run_with_timestamp(timestamp + i).await.unwrap();
                        tokio::time::sleep(Duration::from_millis(25)).await;
                        STATE.with(|state| {
                            let state = state.lock();
                            assert!(state.get(&id).cloned().unwrap_or_default().is_empty());
                            assert_eq!(1, scheduler.pending_tasks.lock().len());
                        });
                    }

                    scheduler.run_with_timestamp(timestamp + 11).await.unwrap();
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    STATE.with(|state| {
                        let state = state.lock();
                        let messages = state.get(&id).cloned().unwrap_or_default();
                        assert_eq!(messages, vec![format!("{} - StepOne", id),]);
                        assert!(scheduler.pending_tasks.lock().is_empty());
                    });
                })
                .await;
        }

        fn scheduler() -> Scheduler<
            SimpleTask,
            StableUnboundedMap<
                u32,
                ScheduledTask<SimpleTask>,
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            >,
            VirtualMemory<DefaultMemoryImpl>,
        > {
            let map: StableUnboundedMap<
                u32,
                ScheduledTask<SimpleTask>,
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            > = StableUnboundedMap::new(VectorMemory::default());
            let memory_manager: ic_stable_structures::IcMemoryManager<
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            > = default_ic_memory_manager();
            Scheduler::new(map, &memory_manager, 1).unwrap()
        }
    }

    mod test_failure_and_retry {

        use std::collections::HashMap;
        use std::future::Future;
        use std::pin::Pin;
        use std::time::Duration;

        use ic_stable_structures::{StableUnboundedMap, VectorMemory};
        use rand::random;
        use serde::{Deserialize, Serialize};

        use super::*;
        use crate::task::TaskOptions;

        #[derive(Default, Clone)]
        struct Output {
            messages: Vec<String>,
            failures: u32,
        }

        thread_local! {
            static STATE: Mutex<HashMap<u32, Output>> = Mutex::new(HashMap::new());
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub enum SimpleTask {
            StepOne { id: u32, fails: u32 },
        }

        impl Task for SimpleTask {
            fn execute(
                &self,
                _task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
            ) -> Pin<Box<dyn Future<Output = Result<()>>>> {
                match self {
                    SimpleTask::StepOne { id, fails } => {
                        let id = *id;
                        let fails = *fails;
                        Box::pin(async move {
                            STATE.with(|state| {
                                let mut state = state.lock();
                                let output = state.entry(id).or_default();
                                if fails > output.failures {
                                    output.failures += 1;
                                    let msg =
                                        format!("{} - StepOne - Failure {}", id, output.failures);
                                    println!("{}", msg);
                                    output.messages.push(msg);
                                    Err(SchedulerError::TaskExecutionFailed("".into()))
                                } else {
                                    let msg = format!("{} - StepOne - Success", id);
                                    println!("{}", msg);
                                    output.messages.push(msg);
                                    Ok(())
                                }
                            })
                        })
                    }
                }
            }
        }

        #[tokio::test]
        async fn test_task_failure_and_retry() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let scheduler = scheduler();
                    let id = random();
                    let fails = 10;
                    let retries = 3;

                    scheduler.append_task(
                        (
                            SimpleTask::StepOne { id, fails },
                            TaskOptions::new()
                                .with_max_retries_policy(retries)
                                .with_fixed_backoff_policy(0),
                        )
                            .into(),
                    );

                    // beware that the the first execution is not a retry
                    for i in 1..=retries {
                        scheduler.run().await.unwrap();
                        tokio::time::sleep(Duration::from_millis(25)).await;
                        STATE.with(|state| {
                            let state = state.lock();
                            let output = state.get(&id).cloned().unwrap_or_default();
                            assert_eq!(output.failures, i);
                            assert_eq!(output.messages.len(), i as usize);
                            assert_eq!(
                                output.messages.last(),
                                Some(&format!("{} - StepOne - Failure {}", id, i))
                            );
                        });
                        let pending_tasks = scheduler.pending_tasks.lock();
                        assert_eq!(pending_tasks.len(), 1);
                        assert_eq!(pending_tasks.get(&0).unwrap().options.failures, i);
                    }

                    // After the last retries the task is removed
                    scheduler.run().await.unwrap();
                    tokio::time::sleep(Duration::from_millis(25)).await;

                    STATE.with(|state| {
                        let state = state.lock();
                        let output = state.get(&id).cloned().unwrap_or_default();
                        assert_eq!(output.failures, 4);
                        assert_eq!(
                            output.messages,
                            vec![
                                format!("{} - StepOne - Failure 1", id),
                                format!("{} - StepOne - Failure 2", id),
                                format!("{} - StepOne - Failure 3", id),
                                format!("{} - StepOne - Failure 4", id),
                            ]
                        );
                        assert_eq!(scheduler.pending_tasks.lock().len(), 0);
                    });
                })
                .await;
        }

        #[tokio::test]
        async fn test_task_succeeds_if_more_retries_than_failures() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let scheduler = scheduler();
                    let id = random();
                    let fails = 2;
                    let retries = 4;

                    scheduler.append_task(
                        (
                            SimpleTask::StepOne { id, fails },
                            TaskOptions::new()
                                .with_max_retries_policy(retries)
                                .with_fixed_backoff_policy(0),
                        )
                            .into(),
                    );

                    // beware that the the first execution is not a retry
                    for _ in 1..=retries {
                        scheduler.run().await.unwrap();
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }

                    STATE.with(|state| {
                        let state = state.lock();
                        let output = state.get(&id).cloned().unwrap_or_default();
                        assert_eq!(
                            output.messages,
                            vec![
                                format!("{} - StepOne - Failure 1", id),
                                format!("{} - StepOne - Failure 2", id),
                                format!("{} - StepOne - Success", id),
                            ]
                        );
                        assert_eq!(scheduler.pending_tasks.lock().len(), 0);
                    });
                })
                .await;
        }

        #[tokio::test]
        async fn test_task_retry_delay() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let scheduler = scheduler();
                    let id = random();
                    let fails = 10;
                    let retries = 10;
                    let retry_delay_secs = 3u64;

                    scheduler.append_task(
                        (
                            SimpleTask::StepOne { id, fails },
                            TaskOptions::new()
                                .with_max_retries_policy(retries)
                                .with_fixed_backoff_policy(retry_delay_secs as u32),
                        )
                            .into(),
                    );

                    let timestamp = random();
                    assert_eq!(1, scheduler.run_with_timestamp(timestamp).await.unwrap());
                    tokio::time::sleep(Duration::from_millis(25)).await;

                    {
                        let pending_tasks = scheduler.pending_tasks.lock();
                        assert_eq!(pending_tasks.len(), 1);
                        assert_eq!(pending_tasks.get(&0).unwrap().options.failures, 1);
                        assert_eq!(
                            pending_tasks
                                .get(&0)
                                .unwrap()
                                .options
                                .execute_after_timestamp_in_secs,
                            timestamp + retry_delay_secs
                        );
                    }

                    // Should not run the task because the retry timestamp is in the future
                    for i in 0..retry_delay_secs {
                        assert_eq!(
                            0,
                            scheduler.run_with_timestamp(timestamp + i).await.unwrap()
                        );
                    }

                    assert_eq!(
                        1,
                        scheduler
                            .run_with_timestamp(timestamp + retry_delay_secs)
                            .await
                            .unwrap()
                    );
                })
                .await;
        }

        #[tokio::test]
        async fn test_should_call_error_cb() {
            use std::sync::atomic::AtomicBool;

            let local = tokio::task::LocalSet::new();
            let called = Arc::new(AtomicBool::new(false));
            let called_t = called.clone();
            local
                .run_until(async move {
                    let mut scheduler = scheduler();

                    scheduler.set_failed_task_callback(move |_, _| {
                        called_t.store(true, std::sync::atomic::Ordering::SeqCst);
                    });

                    let id = random();
                    let fails = 10;

                    scheduler.append_task(
                        (
                            SimpleTask::StepOne { id, fails },
                            TaskOptions::new().with_fixed_backoff_policy(0),
                        )
                            .into(),
                    );

                    // beware that the the first execution is not a retry
                    scheduler.run().await.unwrap();
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    let pending_tasks = scheduler.pending_tasks.lock();
                    assert_eq!(pending_tasks.len(), 0);
                })
                .await;
            assert!(called.load(std::sync::atomic::Ordering::SeqCst));
        }

        #[tokio::test]
        async fn test_should_not_call_error_cb_if_succeeds_after_retries() {
            use std::sync::atomic::AtomicBool;

            let local = tokio::task::LocalSet::new();
            let called = Arc::new(AtomicBool::new(false));
            let called_t = called.clone();
            local
                .run_until(async move {
                    let mut scheduler = scheduler();

                    scheduler.set_failed_task_callback(move |_, _| {
                        called_t.store(true, std::sync::atomic::Ordering::SeqCst);
                    });

                    let id = random();
                    let fails = 2;
                    let retries = 4;

                    scheduler.append_task(
                        (
                            SimpleTask::StepOne { id, fails },
                            TaskOptions::new()
                                .with_max_retries_policy(retries)
                                .with_fixed_backoff_policy(0),
                        )
                            .into(),
                    );

                    // beware that the the first execution is not a retry
                    for _ in 1..=retries {
                        scheduler.run().await.unwrap();
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }

                    STATE.with(|state| {
                        let state = state.lock();
                        let output = state.get(&id).cloned().unwrap_or_default();
                        assert_eq!(
                            output.messages,
                            vec![
                                format!("{} - StepOne - Failure 1", id),
                                format!("{} - StepOne - Failure 2", id),
                                format!("{} - StepOne - Success", id),
                            ]
                        );
                        assert_eq!(scheduler.pending_tasks.lock().len(), 0);
                    });
                })
                .await;
            assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
        }

        #[tokio::test]
        async fn test_should_call_error_only_after_retries() {
            use std::sync::atomic::AtomicU8;

            let local = tokio::task::LocalSet::new();
            let called = Arc::new(AtomicU8::new(0));
            let called_t = called.clone();
            local
                .run_until(async move {
                    let mut scheduler = scheduler();

                    scheduler.set_failed_task_callback(move |_, _| {
                        called_t.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    });

                    let id = random();
                    let fails = 10;
                    let retries = 3;

                    scheduler.append_task(
                        (
                            SimpleTask::StepOne { id, fails },
                            TaskOptions::new()
                                .with_max_retries_policy(retries)
                                .with_fixed_backoff_policy(0),
                        )
                            .into(),
                    );

                    // beware that the the first execution is not a retry
                    for i in 1..=retries {
                        scheduler.run().await.unwrap();
                        tokio::time::sleep(Duration::from_millis(25)).await;
                        STATE.with(|state| {
                            let state = state.lock();
                            let output = state.get(&id).cloned().unwrap_or_default();
                            assert_eq!(output.failures, i);
                            assert_eq!(output.messages.len(), i as usize);
                            assert_eq!(
                                output.messages.last(),
                                Some(&format!("{} - StepOne - Failure {}", id, i))
                            );
                        });
                        let pending_tasks = scheduler.pending_tasks.lock();
                        assert_eq!(pending_tasks.len(), 1);
                        assert_eq!(pending_tasks.get(&0).unwrap().options.failures, i);
                    }

                    // After the last retries the task is removed
                    scheduler.run().await.unwrap();
                    tokio::time::sleep(Duration::from_millis(25)).await;

                    STATE.with(|state| {
                        let state = state.lock();
                        let output = state.get(&id).cloned().unwrap_or_default();
                        assert_eq!(output.failures, 4);
                        assert_eq!(
                            output.messages,
                            vec![
                                format!("{} - StepOne - Failure 1", id),
                                format!("{} - StepOne - Failure 2", id),
                                format!("{} - StepOne - Failure 3", id),
                                format!("{} - StepOne - Failure 4", id),
                            ]
                        );
                        assert_eq!(scheduler.pending_tasks.lock().len(), 0);
                    });
                })
                .await;
            assert_eq!(called.load(std::sync::atomic::Ordering::SeqCst), 1);
        }

        fn scheduler() -> Scheduler<
            SimpleTask,
            StableUnboundedMap<
                u32,
                ScheduledTask<SimpleTask>,
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            >,
            VirtualMemory<DefaultMemoryImpl>,
        > {
            let map: StableUnboundedMap<
                u32,
                ScheduledTask<SimpleTask>,
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            > = StableUnboundedMap::new(VectorMemory::default());
            let memory_manager: ic_stable_structures::IcMemoryManager<
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            > = default_ic_memory_manager();
            Scheduler::new(map, &memory_manager, 1).unwrap()
        }
    }
}
