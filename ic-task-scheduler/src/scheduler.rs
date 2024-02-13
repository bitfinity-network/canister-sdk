use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use ic_kit::RejectionCode;
use ic_stable_structures::{BTreeMapStructure as _, HeapBTreeMap, IterableUnboundedMapStructure};
use parking_lot::Mutex;

use crate::task::{ScheduledTask, Task};
use crate::time::time_secs;
use crate::{Result, SchedulerError};

/// Internal type used to store tasks to be processed and processing tasks
type TaskQueue = Arc<Mutex<HeapBTreeMap<u32, (), ()>>>;

/// The state of a task execution.
/// This is reported when the `SaveStateQueryCallback` is called.
#[derive(Debug)]
pub enum TaskExecutionState {
    /// Reported when tasks to be executed are scheduled
    Scheduled,
    /// Reported when a task fails.
    Failed(u32, SchedulerError),
    /// Reported when a task starts executing.
    Executing(u32),
    /// Reported when a task completes successfully.
    Completed(u32),
    /// Reported when a task panics.
    Panicked(u32),
}

type OnStateChangeCallback = dyn Fn(
        TaskExecutionState,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), (RejectionCode, String)>>>>
    + Send
    + Sync;

/// A scheduler is responsible for executing tasks.
pub struct Scheduler<T, P>
where
    T: 'static + Task,
    P: 'static + IterableUnboundedMapStructure<u32, ScheduledTask<T>>,
{
    pending_tasks: Arc<Mutex<P>>,
    phantom: std::marker::PhantomData<T>,
    /// Queue containing tasks to be processed
    tasks_to_be_processed: TaskQueue,
    /// Tasks which are currently being processed
    tasks_running: TaskQueue,
    /// Callback to be called to save the current canister state to prevent panicking tasks.
    on_execution_state_changed_callback: Arc<Box<OnStateChangeCallback>>,
}

impl<T, P> Scheduler<T, P>
where
    T: 'static + Task,
    P: 'static + IterableUnboundedMapStructure<u32, ScheduledTask<T>>,
{
    /// Create a new scheduler.
    ///
    /// A callback `on_execution_state_changed_callback` is called every time the state of a task is changed.
    /// By performing an inter-canister call in the callback, you can force the state to be persisted even in case of
    /// panics. This allows the scheduler to deal with panicking tasks.
    pub fn new(
        pending_tasks: P,
        on_execution_state_changed_callback: Box<OnStateChangeCallback>,
    ) -> Result<Self> {
        Ok(Self {
            pending_tasks: Arc::new(Mutex::new(pending_tasks)),
            phantom: std::marker::PhantomData,
            on_execution_state_changed_callback: Arc::new(on_execution_state_changed_callback),
            tasks_to_be_processed: Arc::new(Mutex::new(HeapBTreeMap::new(()))),
            tasks_running: Arc::new(Mutex::new(HeapBTreeMap::new(()))),
        })
    }

    /// Execute all pending tasks.
    /// Each task is executed asynchronously in a dedicated ic_cdk::spawn call.
    /// This function does not wait for the tasks to complete.
    /// Returns the number of tasks that have been launched.
    pub async fn run(&mut self) -> Result<u32> {
        self.run_with_timestamp(time_secs()).await
    }

    async fn run_with_timestamp(&mut self, now_timestamp_secs: u64) -> Result<u32> {
        let mut task_execution_started = 0;

        // checks tasks that are still in tasks running (something bad happened in the last cycle)
        let tasks_running_count = self.tasks_running.lock().len();
        match tasks_running_count {
            0 => {
                // HAPPY PATH: if there are no processing tasks, initialize the tasks o be processed
                self.init_tasks_to_be_processed(now_timestamp_secs).await?;
            }
            1 => {
                // if there is only one processing task, we can assume that it panicked
                // delete that task and mark it as panicked
                let task = self.tasks_running.lock().iter().next().unwrap().0;
                self.delete_unprocessable_task(task).await?;

                // eventually reschedule the tasks to be processed
                self.init_tasks_to_be_processed(now_timestamp_secs).await?;
            }
            _ => {
                // if there is more than one task to be processed, keep only half of them
                self.split_processing_tasks().await?;
            }
        }

        // iterate over tasks to be processed, and execute it one by one
        let tasks_to_be_processed: Vec<u32> = self
            .tasks_to_be_processed
            .lock()
            .iter()
            .map(|(key, _)| key)
            .collect();
        for task_id in tasks_to_be_processed {
            let lock = self.pending_tasks.lock();
            let mut task = match lock.get(&task_id) {
                Some(task) => task,
                None => continue,
            };
            drop(lock);

            task_execution_started += 1;
            let key = task_id;
            let mut task_scheduler = self.clone();
            Self::spawn(async move {
                // put task to processing tasks
                task_scheduler
                    .put_task_to_processing_tasks(key)
                    .await
                    .unwrap();

                // execute the task
                if let Err(err) = task.task.execute(Box::new(task_scheduler.clone())).await {
                    task.options.failures += 1;
                    let (should_retry, retry_delay) = task
                        .options
                        .retry_strategy
                        .should_retry(task.options.failures);
                    if should_retry {
                        // remove task from processing task, but don't report state
                        task_scheduler.remove_task_from_processing_tasks(key);

                        // re-add task to the queue
                        task.options.execute_after_timestamp_in_secs =
                            now_timestamp_secs + (retry_delay as u64);
                        task_scheduler.append_task(task.clone())
                    } else {
                        // remove task from processing and port its failure
                        task_scheduler
                            .remove_failed_task_from_processing_tasks(key, err)
                            .await
                            .unwrap();
                    }
                } else {
                    // in case of success, remove task from queue and report success
                    task_scheduler
                        .remove_completed_task_from_processing_tasks(key)
                        .await
                        .unwrap();
                }
            });
        }

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

    /// Copy `tasks_being_processed` into `tasks_to_be_processed`,
    ///
    /// then keep in the hashset only the first half of the tasks.
    async fn split_processing_tasks(&mut self) -> Result<()> {
        // move tasks_being_processed to tasks_to_be_processed
        {
            let mut tasks_running_lock = self.tasks_running.lock();
            let mut tasks_to_be_processed_lock = self.tasks_running.lock();
            tasks_to_be_processed_lock.clear();
            for (key, _) in tasks_running_lock.iter() {
                tasks_to_be_processed_lock.insert(key, ());
            }
            // clear tasks_running
            tasks_running_lock.clear();
            drop(tasks_running_lock);

            // remove second half
            let total_tasks_half = (tasks_to_be_processed_lock.len() / 2) as usize;
            let second_half: Vec<u32> = tasks_to_be_processed_lock
                .iter()
                .enumerate()
                .filter_map(|(i, (task, _))| {
                    if i > total_tasks_half {
                        Some(task)
                    } else {
                        None
                    }
                })
                .collect();

            for task in second_half {
                tasks_to_be_processed_lock.remove(&task);
            }
            drop(tasks_to_be_processed_lock);
        }

        // save state
        self.report_state(TaskExecutionState::Scheduled).await
    }

    /// Initialize tasks to be processed using the current timestamp and checking against the tasks which must
    /// executed in this slot.
    ///
    /// Then reset processing tasks to an empty set
    async fn init_tasks_to_be_processed(&mut self, timestamp: u64) -> Result<()> {
        {
            // save tasks to be executed at this time
            let tasks_to_be_executed: Vec<u32> = self
                .pending_tasks
                .lock()
                .iter()
                .filter_map(|(key, task)| {
                    if task.options.execute_after_timestamp_in_secs <= timestamp {
                        Some(key)
                    } else {
                        None
                    }
                })
                .collect();

            // clear and insert tasks
            let mut tasks_to_be_processed_lock = self.tasks_to_be_processed.lock();
            tasks_to_be_processed_lock.clear();
            for task in tasks_to_be_executed {
                tasks_to_be_processed_lock.insert(task, ());
            }
            drop(tasks_to_be_processed_lock);

            self.tasks_running.lock().clear();
        }
        // save state
        self.report_state(TaskExecutionState::Scheduled).await
    }

    /// Remove a task from `to_be_processed` and move it into the `processing` set. Then save the current task
    async fn put_task_to_processing_tasks(&mut self, task: u32) -> Result<()> {
        let task_removed = self.tasks_to_be_processed.lock().remove(&task).is_some();

        if task_removed {
            self.tasks_running.lock().insert(task, ());
            // save state
            self.report_state(TaskExecutionState::Executing(task)).await
        } else {
            Ok(())
        }
    }

    /// Remove a task from the tasks queue and save the state marking it as completed
    async fn remove_completed_task_from_processing_tasks(&mut self, task: u32) -> Result<()> {
        self.remove_task_from_processing_tasks(task);
        // save state
        self.report_state(TaskExecutionState::Completed(task)).await
    }

    /// Remove a failed task (FAILED! NOT PANICKED) from the tasks queue and save the state marking it as completed
    async fn remove_failed_task_from_processing_tasks(
        &mut self,
        task: u32,
        error: SchedulerError,
    ) -> Result<()> {
        self.remove_task_from_processing_tasks(task);

        // save state
        self.report_state(TaskExecutionState::Failed(task, error))
            .await
    }

    /// Remove a task from the tasks queue
    fn remove_task_from_processing_tasks(&mut self, task: u32) {
        // delete the task from tasks_running
        self.tasks_running.lock().remove(&task);
        // delete the task from pending_tasks
        self.pending_tasks.lock().remove(&task);
    }

    /// Remove a task from `tasks_running` and from `pending_tasks`
    async fn delete_unprocessable_task(&mut self, task: u32) -> Result<()> {
        // delete the task from tasks_running
        self.tasks_running.lock().remove(&task);
        // delete the task from pending_tasks
        self.pending_tasks.lock().remove(&task);
        // save state
        self.report_state(TaskExecutionState::Panicked(task)).await
    }

    /// Save the current state of the scheduler.
    async fn report_state(&self, state: TaskExecutionState) -> Result<()> {
        (*self.on_execution_state_changed_callback)(state).await?;

        Ok(())
    }
}

pub trait TaskScheduler<T: 'static + Task> {
    fn append_task(&self, task: ScheduledTask<T>);
    fn append_tasks(&self, tasks: Vec<ScheduledTask<T>>);
}

impl<T, P> Clone for Scheduler<T, P>
where
    T: 'static + Task,
    P: 'static + IterableUnboundedMapStructure<u32, ScheduledTask<T>>,
{
    fn clone(&self) -> Self {
        Self {
            pending_tasks: self.pending_tasks.clone(),
            phantom: self.phantom,
            tasks_running: self.tasks_running.clone(),
            tasks_to_be_processed: self.tasks_to_be_processed.clone(),
            on_execution_state_changed_callback: self.on_execution_state_changed_callback.clone(),
        }
    }
}

impl<T, P> TaskScheduler<T> for Scheduler<T, P>
where
    T: 'static + Task,
    P: 'static + IterableUnboundedMapStructure<u32, ScheduledTask<T>>,
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
#[allow(clippy::type_complexity)]
mod test {

    use super::*;

    type SaveStateCb =
        Pin<Box<dyn Future<Output = std::result::Result<(), (RejectionCode, String)>>>>;

    mod test_execution {

        use std::cell::RefCell;
        use std::collections::HashMap;
        use std::future::Future;
        use std::pin::Pin;
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

        thread_local! {
            static REPORT_STATE_CB_CALLED: RefCell<Option<TaskExecutionState>> = RefCell::new(None);
        }

        #[tokio::test]
        async fn test_should_call_report_state_cb() {
            let scheduler = scheduler();

            assert!(scheduler
                .report_state(TaskExecutionState::Failed(
                    1,
                    SchedulerError::TaskExecutionFailed("ciao".to_string())
                ))
                .await
                .is_ok());

            REPORT_STATE_CB_CALLED.with_borrow(|state| {
                assert!(matches!(
                    state.as_ref().unwrap(),
                    TaskExecutionState::Failed(1, _)
                ));
            });
        }

        async fn report_state(
            state: TaskExecutionState,
        ) -> std::result::Result<(), (RejectionCode, String)> {
            if let TaskExecutionState::Failed(id, err) = state {
                REPORT_STATE_CB_CALLED.with(|called| {
                    called.replace(Some(TaskExecutionState::Failed(id, err)));
                });
            }

            Ok(())
        }

        fn report_state_cb(state: TaskExecutionState) -> SaveStateCb {
            Box::pin(async { report_state(state).await })
        }

        #[tokio::test]
        async fn test_run_scheduler() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let mut scheduler = scheduler();
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

            local
                .run_until(async move {
                    let mut scheduler = scheduler();
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

            REPORT_STATE_CB_CALLED.with_borrow(|state| {
                assert!(state.is_none());
            });
        }

        fn scheduler() -> Scheduler<
            SimpleTaskSteps,
            StableUnboundedMap<
                u32,
                ScheduledTask<SimpleTaskSteps>,
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            >,
        > {
            let map: StableUnboundedMap<
                u32,
                ScheduledTask<SimpleTaskSteps>,
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            > = StableUnboundedMap::new(VectorMemory::default());
            Scheduler::new(map, Box::new(report_state_cb)).unwrap()
        }
    }

    mod test_delay {

        use std::cell::RefCell;
        use std::collections::HashMap;
        use std::future::Future;
        use std::pin::Pin;
        use std::time::Duration;

        use ic_stable_structures::{StableUnboundedMap, UnboundedMapStructure as _, VectorMemory};
        use rand::random;
        use serde::{Deserialize, Serialize};

        use super::*;
        use crate::task::TaskOptions;

        thread_local! {
            pub static STATE: Mutex<HashMap<u32, Vec<String>>> = Mutex::new(HashMap::new());

            static REPORT_STATE_CB_CALLED: RefCell<Option<TaskExecutionState>> = RefCell::new(None);
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
                    let mut scheduler = scheduler();
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
        > {
            let map: StableUnboundedMap<
                u32,
                ScheduledTask<SimpleTask>,
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            > = StableUnboundedMap::new(VectorMemory::default());

            Scheduler::new(map, Box::new(report_state_cb)).unwrap()
        }

        async fn report_state(
            state: TaskExecutionState,
        ) -> std::result::Result<(), (RejectionCode, String)> {
            if let TaskExecutionState::Failed(id, err) = state {
                REPORT_STATE_CB_CALLED.with(|called| {
                    called.replace(Some(TaskExecutionState::Failed(id, err)));
                });
            }
            Ok(())
        }

        fn report_state_cb(state: TaskExecutionState) -> SaveStateCb {
            Box::pin(async { report_state(state).await })
        }
    }

    mod test_failure_and_retry {

        use std::cell::RefCell;
        use std::collections::HashMap;
        use std::future::Future;
        use std::pin::Pin;
        use std::time::Duration;

        use ic_stable_structures::{StableUnboundedMap, UnboundedMapStructure as _, VectorMemory};
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

            static REPORT_STATE_CB_CALLED: RefCell<Option<TaskExecutionState>> = RefCell::new(None);
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
                    let mut scheduler = scheduler();
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
                    let mut scheduler = scheduler();
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
                    let mut scheduler = scheduler();
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
            let local = tokio::task::LocalSet::new();
            let id = random();
            local
                .run_until(async move {
                    let mut scheduler = scheduler();

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
            REPORT_STATE_CB_CALLED.with_borrow(|state| {
                assert!(matches!(
                    state.as_ref().unwrap(),
                    TaskExecutionState::Failed(_, _)
                ))
            });
        }

        #[tokio::test]
        async fn test_should_not_call_error_cb_if_succeeds_after_retries() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let mut scheduler = scheduler();

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

            REPORT_STATE_CB_CALLED.with_borrow(|state| {
                assert!(state.is_none());
            });
        }

        #[tokio::test]
        async fn test_should_call_error_only_after_retries() {
            let local = tokio::task::LocalSet::new();
            let id = random();
            local
                .run_until(async move {
                    let mut scheduler = scheduler();

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
                        println!(
                            "{:?}",
                            pending_tasks
                                .iter()
                                .map(|(k, v)| (k, v.options.failures))
                                .collect::<Vec<_>>()
                        );
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
            REPORT_STATE_CB_CALLED.with_borrow(|state| {
                assert!(matches!(
                    state.as_ref().unwrap(),
                    TaskExecutionState::Failed(_, _)
                ));
            });
        }

        fn scheduler() -> Scheduler<
            SimpleTask,
            StableUnboundedMap<
                u32,
                ScheduledTask<SimpleTask>,
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            >,
        > {
            let map: StableUnboundedMap<
                u32,
                ScheduledTask<SimpleTask>,
                std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
            > = StableUnboundedMap::new(VectorMemory::default());
            Scheduler::new(map, Box::new(report_state_cb)).unwrap()
        }

        async fn report_state(
            state: TaskExecutionState,
        ) -> std::result::Result<(), (RejectionCode, String)> {
            if let TaskExecutionState::Failed(id, err) = state {
                REPORT_STATE_CB_CALLED.with(|called| {
                    called.replace(Some(TaskExecutionState::Failed(id, err)));
                });
            }
            Ok(())
        }

        fn report_state_cb(state: TaskExecutionState) -> SaveStateCb {
            Box::pin(async { report_state(state).await })
        }
    }
}
