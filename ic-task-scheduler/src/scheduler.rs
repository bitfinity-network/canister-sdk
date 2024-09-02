use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use ic_stable_structures::{BTreeMapStructure, CellStructure, IterableSortedMapStructure};
use log::{debug, warn};
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::task::{InnerScheduledTask, ScheduledTask, Task, TaskOptions, TaskStatus};
use crate::time::time_secs;
use crate::SchedulerError;

type TaskCompletionCallback<T> = Box<dyn 'static + Fn(InnerScheduledTask<T>) + Send>;

const DEFAULT_RUNNING_TASK_TIMEOUT_SECS: u64 = 120;

/// A scheduler is responsible for executing tasks.
pub struct Scheduler<T, P, S>
where
    T: 'static + Task,
    P: 'static
        + IterableSortedMapStructure<u64, InnerScheduledTask<T>>
        + BTreeMapStructure<u64, InnerScheduledTask<T>>,
    S: 'static + CellStructure<u64>,
{
    pending_tasks: Arc<Mutex<P>>,
    phantom: std::marker::PhantomData<T>,
    on_completion_callback: Arc<Option<TaskCompletionCallback<T>>>,
    running_task_timeout_secs: AtomicU64,
    /// The next scheduled task id
    task_id_sequence: Arc<Mutex<S>>,
}

impl<T, P, S> Scheduler<T, P, S>
where
    T: 'static + Task + Serialize + DeserializeOwned + Clone,
    T::Ctx: Clone,
    P: 'static
        + IterableSortedMapStructure<u64, InnerScheduledTask<T>>
        + BTreeMapStructure<u64, InnerScheduledTask<T>>,
    S: 'static + CellStructure<u64>,
{
    /// Create a new scheduler.
    pub fn new(pending_tasks: P, task_id_sequence: S) -> Self {
        Self {
            pending_tasks: Arc::new(Mutex::new(pending_tasks)),
            phantom: std::marker::PhantomData,
            on_completion_callback: Arc::new(None),
            running_task_timeout_secs: AtomicU64::new(DEFAULT_RUNNING_TASK_TIMEOUT_SECS),
            task_id_sequence: Arc::new(Mutex::new(task_id_sequence)),
        }
    }

    /// Set the timeout of a running task. If a task is running for more time the timeout, it will be
    /// considered as stuck or panicked.
    /// The default value is 120 seconds.
    pub fn set_running_task_timeout(&mut self, timeout_secs: u64) {
        debug!("Setting running task timeout to {} seconds", timeout_secs);
        self.running_task_timeout_secs
            .store(timeout_secs, Ordering::Relaxed);
    }

    /// Set a callback to be called when a task execution completes.
    pub fn on_completion_callback<F: 'static + Send + Fn(InnerScheduledTask<T>)>(&mut self, cb: F) {
        self.on_completion_callback = Arc::new(Some(Box::new(cb)));
    }

    /// Execute all pending tasks.
    /// Each task is executed asynchronously in a dedicated ic_cdk::spawn call.
    /// This function does not wait for the tasks to complete.
    /// Returns the number of tasks that have been launched.
    pub fn run(&self, ctx: T::Ctx) -> Result<usize, SchedulerError> {
        self.run_with_timestamp(ctx, time_secs())
    }

    fn run_with_timestamp(
        &self,
        context: T::Ctx,
        now_timestamp_secs: u64,
    ) -> Result<usize, SchedulerError> {
        debug!("Scheduler - Running tasks");
        let mut to_be_scheduled_tasks = Vec::new();
        let mut out_of_time_tasks = Vec::new();
        let running_task_timeout_secs = self.running_task_timeout_secs.load(Ordering::Relaxed);

        {
            let lock = self.pending_tasks.lock();
            for (task_key, task) in lock.iter() {
                match task.status {
                    TaskStatus::Waiting { .. } => {
                        if task.options.execute_after_timestamp_in_secs <= now_timestamp_secs {
                            debug!("Scheduler - Task {} scheduled to be processed", task_key);
                            to_be_scheduled_tasks.push(task_key);
                        }
                    }
                    TaskStatus::Running { timestamp_secs }
                    | TaskStatus::Scheduled { timestamp_secs } => {
                        if timestamp_secs + running_task_timeout_secs < now_timestamp_secs {
                            warn!(
                                "Scheduler - Task {} was in Scheduled or Running status for more than {} seconds, it could be stuck or panicked. Removing it from the scheduler.",
                                task_key, running_task_timeout_secs
                            );
                            out_of_time_tasks.push(task_key);
                        }
                    }
                    TaskStatus::Completed { .. }
                    | TaskStatus::TimeoutOrPanic { .. }
                    | TaskStatus::Failed { .. } => (),
                }
            }
        }

        // Process the tasks that are ready to be scheduled
        for task_key in to_be_scheduled_tasks.iter() {
            self.process_pending_task(context.clone(), *task_key, now_timestamp_secs);
        }

        // Remove the tasks that are out of time
        {
            let mut lock = self.pending_tasks.lock();
            for task_key in out_of_time_tasks.into_iter() {
                if let Some(mut task) = lock.remove(&task_key) {
                    task.status = TaskStatus::timeout_or_panic(now_timestamp_secs);
                    if let Some(cb) = &*self.on_completion_callback {
                        cb(task);
                    }
                }
            }
        }

        Ok(to_be_scheduled_tasks.len())
    }

    fn process_pending_task(&self, context: T::Ctx, task_key: u64, now_timestamp_secs: u64) {
        let task_scheduler = self.clone();

        // Set the task as scheduled
        {
            let mut lock = task_scheduler.pending_tasks.lock();
            let task = lock.get(&task_key);
            if let Some(mut task) = task {
                if let TaskStatus::Waiting { .. } = task.status {
                    debug!(
                        "Scheduler - Task {} status changed: Waiting -> Scheduled",
                        task_key
                    );
                    task.status = TaskStatus::scheduled(now_timestamp_secs);
                    lock.insert(task_key, task);
                }
            }
        }

        Self::spawn(async move {
            let now_timestamp_secs = time_secs();

            let task = task_scheduler.pending_tasks.lock().get(&task_key);
            if let Some(mut task) = task {
                if let TaskStatus::Scheduled { .. } = task.status {
                    debug!(
                        "Scheduler - Task {} status changed: Scheduled -> Running",
                        task_key
                    );
                    task.status = TaskStatus::running(now_timestamp_secs);
                    task_scheduler
                        .pending_tasks
                        .lock()
                        .insert(task_key, task.clone());

                    let completed_task = match task
                        .task
                        .execute(context, Box::new(task_scheduler.clone()))
                        .await
                    {
                        Ok(()) => {
                            debug!("Scheduler - Task {} execution succeeded. Status changed: Running -> Completed", task_key);
                            let mut lock = task_scheduler.pending_tasks.lock();
                            let mut task = lock.remove(&task_key).unwrap();
                            task.status = TaskStatus::completed(now_timestamp_secs);
                            Some(task)
                        }
                        Err(err) => {
                            let mut lock = task_scheduler.pending_tasks.lock();
                            if let Some(updated_task) = lock.get(&task.id) {
                                task.options = updated_task.options;
                            }
                            task.options.failures += 1;
                            let (should_retry, retry_delay) = match err {
                                SchedulerError::Unrecoverable(_) => (false, 0),
                                _ => task
                                    .options
                                    .retry_strategy
                                    .should_retry(task.options.failures),
                            };

                            if should_retry {
                                debug!("Scheduler - Task {} execution failed. Execution will be retried. Status changed: Running -> Waiting", task_key);
                                task.options.execute_after_timestamp_in_secs =
                                    now_timestamp_secs + (retry_delay as u64);
                                task.status = TaskStatus::waiting(now_timestamp_secs);
                                lock.insert(task_key, task);
                                None
                            } else {
                                debug!("Scheduler - Task {} execution failed. Status changed: Running -> Failed", task_key);
                                let mut task = lock.remove(&task_key).unwrap();
                                task.status = TaskStatus::failed(now_timestamp_secs, err);
                                Some(task)
                            }
                        }
                    };

                    if let Some(task) = completed_task {
                        if let Some(cb) = &*task_scheduler.on_completion_callback {
                            cb(task);
                        }
                    }
                }
            }
        });
    }

    /// Returns the next task id.
    fn next_task_id(&self) -> u64 {
        let mut lock = self.task_id_sequence.lock();
        let id = *lock.get();
        lock.set(id + 1)
            .expect("Unable to access the stable storage to set the next poll id");
        id
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
        ic_cdk_timers::set_timer(std::time::Duration::from_millis(0), || {
            ic_kit::ic::spawn(future);
        });
    }
}

pub trait TaskScheduler<T: 'static + Task> {
    /// Append a task to the scheduler and return the key of the task.
    fn append_task(&self, task: ScheduledTask<T>) -> u64;

    /// Append a list of tasks to the scheduler and return the keys of the tasks.
    fn append_tasks(&self, tasks: Vec<ScheduledTask<T>>) -> Vec<u64>;

    /// Get a task by its key.
    fn get_task(&self, task_id: u64) -> Option<InnerScheduledTask<T>>;

    /// Returns identifier of the first task for which the `filter` predicate returns true.
    /// This method is used to locate a task id of a specific task.
    ///
    /// NOTE: Iterating over all tasks requires loading them one by once from IC stable memory
    /// (if stable memory is used for scheduler), which can be slow in case there are many pending
    /// tasks in the scheduler.
    fn find_id(&self, filter: &dyn Fn(T) -> bool) -> Option<u64>;

    /// Changes the retry parameters of the given task id to the new `options` value.
    ///
    /// If the task is currently running, the current execution will be considered as the first
    /// execution of the new retry schedule.
    ///
    /// If the task with `task_id` identifier doesn't exist, does nothing.
    fn reschedule(&self, task_id: u64, options: TaskOptions);
}

impl<T, P, S> Clone for Scheduler<T, P, S>
where
    T: 'static + Task + Serialize + DeserializeOwned,
    P: 'static
        + IterableSortedMapStructure<u64, InnerScheduledTask<T>>
        + BTreeMapStructure<u64, InnerScheduledTask<T>>,
    S: 'static + CellStructure<u64>,
{
    fn clone(&self) -> Self {
        Self {
            pending_tasks: self.pending_tasks.clone(),
            phantom: self.phantom,
            on_completion_callback: self.on_completion_callback.clone(),
            running_task_timeout_secs: AtomicU64::new(
                self.running_task_timeout_secs.load(Ordering::Relaxed),
            ),
            task_id_sequence: self.task_id_sequence.clone(),
        }
    }
}

impl<T, P, S> TaskScheduler<T> for Scheduler<T, P, S>
where
    T: 'static + Task + Serialize + DeserializeOwned + Clone,
    T::Ctx: Clone,
    P: 'static
        + IterableSortedMapStructure<u64, InnerScheduledTask<T>>
        + BTreeMapStructure<u64, InnerScheduledTask<T>>,
    S: 'static + CellStructure<u64>,
{
    fn append_task(&self, task: ScheduledTask<T>) -> u64 {
        let time_secs = time_secs();
        let mut lock = self.pending_tasks.lock();
        let key = self.next_task_id();
        lock.insert(
            key,
            InnerScheduledTask::with_status(
                key,
                task,
                TaskStatus::Waiting {
                    timestamp_secs: time_secs,
                },
            ),
        );
        key
    }

    fn append_tasks(&self, tasks: Vec<ScheduledTask<T>>) -> Vec<u64> {
        if tasks.is_empty() {
            return vec![];
        };
        let mut keys = Vec::with_capacity(tasks.len());
        for task in tasks {
            keys.push(self.append_task(task));
        }        
        keys
    }

    fn get_task(&self, task_id: u64) -> Option<InnerScheduledTask<T>> {
        self.pending_tasks.lock().get(&task_id)
    }

    fn reschedule(&self, task_id: u64, options: TaskOptions) {
        let mut lock = self.pending_tasks.lock();
        let Some(mut task) = lock.get(&task_id) else {
            return;
        };

        task.options = options;
        lock.insert(task_id, task);
    }

    fn find_id(&self, filter: &dyn Fn(T) -> bool) -> Option<u64> {
        self.pending_tasks.lock().iter().find_map(
            |(id, task)| {
                if filter(task.task) {
                    Some(id)
                } else {
                    None
                }
            },
        )
    }
}

#[cfg(test)]
mod test {

    use super::*;

    mod test_execution {
        use std::cell::RefCell;
        use std::collections::HashMap;
        use std::future::Future;
        use std::pin::Pin;
        use std::rc::Rc;
        use std::sync::atomic::AtomicBool;
        use std::time::Duration;

        use ic_stable_structures::{StableBTreeMap, StableCell, VectorMemory};
        use rand::random;
        use serde::{Deserialize, Serialize};

        use super::*;

        thread_local! {
            pub static STATE: Mutex<HashMap<u64, Vec<String>>> = Mutex::new(HashMap::new())
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub enum SimpleTaskSteps {
            One { id: u64 },
            Two { id: u64 },
            Three { id: u64 },
        }

        impl Task for SimpleTaskSteps {
            type Ctx = Rc<RefCell<u64>>;

            fn execute(
                &self,
                ctx: Rc<RefCell<u64>>,
                task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
            ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
                *ctx.borrow_mut() += 1;

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
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let scheduler = Scheduler::new(map, sequence);
                    let id = random();
                    scheduler.append_task(SimpleTaskSteps::One { id }.into());

                    let mut completed = false;

                    let ctx = Rc::new(RefCell::new(0u64));

                    while !completed {
                        scheduler.run(ctx.clone()).unwrap();
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

                                assert_eq!(*ctx.borrow(), 4u64);
                            }
                        });
                    }
                })
                .await
        }

        #[tokio::test]
        async fn test_error_cb_called_on_success() {
            let local = tokio::task::LocalSet::new();
            let called = Arc::new(AtomicBool::new(false));
            let called_t = called.clone();
            local
                .run_until(async move {
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let mut scheduler = Scheduler::new(map, sequence);
                    scheduler.on_completion_callback(move |task| {
                        if let TaskStatus::Completed { .. } = task.status {
                            called_t.store(true, std::sync::atomic::Ordering::SeqCst);
                        }
                    });
                    let id = random();
                    scheduler.append_task(SimpleTaskSteps::One { id }.into());

                    let mut completed = false;

                    let ctx = Rc::new(RefCell::new(0));

                    while !completed {
                        scheduler.run(ctx.clone()).unwrap();
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

            assert!(called.load(std::sync::atomic::Ordering::SeqCst));
        }
    }

    mod test_delay {
        use std::collections::HashMap;
        use std::future::Future;
        use std::pin::Pin;
        use std::time::Duration;

        use ic_stable_structures::{StableBTreeMap, StableCell, VectorMemory};
        use rand::random;
        use serde::{Deserialize, Serialize};

        use super::*;
        use crate::task::TaskOptions;

        thread_local! {
            pub static STATE: Mutex<HashMap<u64, Vec<String>>> = Mutex::new(HashMap::new())
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub enum SimpleTask {
            StepOne { id: u64 },
        }

        impl Task for SimpleTask {
            type Ctx = ();

            fn execute(
                &self,
                _: Self::Ctx,
                _task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
            ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
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
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let scheduler = Scheduler::new(map, sequence);
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
                        scheduler.run_with_timestamp((), timestamp + i).unwrap();
                        tokio::time::sleep(Duration::from_millis(25)).await;
                        STATE.with(|state| {
                            let state = state.lock();
                            assert!(state.get(&id).cloned().unwrap_or_default().is_empty());
                            assert_eq!(1, scheduler.pending_tasks.lock().len());
                        });
                    }

                    scheduler.run_with_timestamp((), timestamp + 11).unwrap();
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
    }

    mod test_failure_and_retry {

        use std::collections::HashMap;
        use std::future::Future;
        use std::pin::Pin;
        use std::time::Duration;

        use ic_stable_structures::{StableBTreeMap, StableCell, VectorMemory};
        use rand::random;
        use serde::{Deserialize, Serialize};

        use super::*;
        use crate::retry::RetryPolicy;
        use crate::task::TaskOptions;

        #[derive(Default, Clone)]
        struct Output {
            messages: Vec<String>,
            failures: u32,
        }

        thread_local! {
            static STATE: Mutex<HashMap<u64, Output>> = Mutex::new(HashMap::new());
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub enum SimpleTask {
            StepOne { id: u64, fails: u32 },
        }

        impl Task for SimpleTask {
            type Ctx = ();

            fn execute(
                &self,
                _: Self::Ctx,
                _task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
            ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
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

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub struct UnrecoverableTask {
            id: u64,
            tries_before_failure: u32,
        }

        impl Task for UnrecoverableTask {
            type Ctx = ();

            fn execute(
                &self,
                _: Self::Ctx,
                _task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
            ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
                let id = self.id;
                let tries_before_failure = self.tries_before_failure;
                Box::pin(async move {
                    STATE.with(|state| {
                        let mut state = state.lock();
                        let output = state.entry(id).or_default();
                        if output.failures >= tries_before_failure {
                            Err(SchedulerError::Unrecoverable("".into()))
                        } else {
                            output.failures += 1;
                            Err(SchedulerError::TaskExecutionFailed("".into()))
                        }
                    })
                })
            }
        }

        #[tokio::test]
        async fn test_task_failure_and_retry() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let scheduler = Scheduler::new(map, sequence);
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
                        scheduler.run(()).unwrap();
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
                    scheduler.run(()).unwrap();
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
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let scheduler = Scheduler::new(map, sequence);
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
                        scheduler.run(()).unwrap();
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
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let scheduler = Scheduler::new(map, sequence);
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

                    let timestamp = time_secs();
                    assert_eq!(1, scheduler.run_with_timestamp((), timestamp).unwrap());
                    tokio::time::sleep(Duration::from_millis(25)).await;

                    {
                        let pending_tasks = scheduler.pending_tasks.lock();
                        assert_eq!(pending_tasks.len(), 1);
                        assert_eq!(pending_tasks.get(&0).unwrap().options.failures, 1);
                        assert!(
                            pending_tasks
                                .get(&0)
                                .unwrap()
                                .options
                                .execute_after_timestamp_in_secs
                                >= timestamp + retry_delay_secs
                        );
                    }

                    // Should not run the task because the retry timestamp is in the future
                    for i in 0..retry_delay_secs {
                        assert_eq!(0, scheduler.run_with_timestamp((), timestamp + i).unwrap());
                    }

                    assert_eq!(
                        1,
                        scheduler
                            .run_with_timestamp((), timestamp + retry_delay_secs)
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
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let mut scheduler = Scheduler::new(map, sequence);

                    scheduler.on_completion_callback(move |task| {
                        if let TaskStatus::Failed { .. } = task.status {
                            called_t.store(true, std::sync::atomic::Ordering::SeqCst);
                        }
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
                    scheduler.run(()).unwrap();
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
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let mut scheduler = Scheduler::new(map, sequence);

                    scheduler.on_completion_callback(move |task| {
                        if let TaskStatus::Completed { .. } = task.status {
                            called_t.store(true, std::sync::atomic::Ordering::SeqCst);
                        }
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
                        scheduler.run(()).unwrap();
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
            assert!(called.load(std::sync::atomic::Ordering::SeqCst));
        }

        #[tokio::test]
        async fn test_should_call_error_only_after_retries() {
            use std::sync::atomic::AtomicU8;

            let local = tokio::task::LocalSet::new();
            let called = Arc::new(AtomicU8::new(0));
            let called_t = called.clone();
            local
                .run_until(async move {
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let mut scheduler = Scheduler::new(map, sequence);

                    scheduler.on_completion_callback(move |_| {
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
                        scheduler.run(()).unwrap();
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
                    scheduler.run(()).unwrap();
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

        #[tokio::test]
        async fn test_should_not_retry_unrecoverable_errors() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let scheduler = Scheduler::new(map, sequence);
                    let id = random();
                    let retries = 10;
                    let retry_delay_secs = 3u32;

                    scheduler.append_task(
                        (
                            UnrecoverableTask {
                                id,
                                tries_before_failure: 0,
                            },
                            TaskOptions::new()
                                .with_max_retries_policy(retries)
                                .with_fixed_backoff_policy(retry_delay_secs),
                        )
                            .into(),
                    );

                    scheduler.run(()).unwrap();
                    tokio::time::sleep(Duration::from_millis(25)).await;

                    let pending_tasks = scheduler.pending_tasks.lock();
                    assert!(pending_tasks.is_empty());
                })
                .await;
        }

        #[tokio::test]
        async fn test_should_not_retry_unrecoverable_errors_after_retries() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let scheduler = Scheduler::new(map, sequence);
                    let id = random();
                    let retries = 10;

                    scheduler.append_task(
                        (
                            UnrecoverableTask {
                                id,
                                tries_before_failure: retries,
                            },
                            TaskOptions::new()
                                .with_retry_policy(RetryPolicy::Infinite)
                                .with_fixed_backoff_policy(0),
                        )
                            .into(),
                    );

                    for _ in 0..retries {
                        scheduler.run(()).unwrap();
                        tokio::time::sleep(Duration::from_millis(25)).await;

                        let pending_tasks = scheduler.pending_tasks.lock();
                        assert!(!pending_tasks.is_empty());
                    }

                    scheduler.run(()).unwrap();
                    tokio::time::sleep(Duration::from_millis(25)).await;

                    let pending_tasks = scheduler.pending_tasks.lock();
                    assert!(pending_tasks.is_empty());
                })
                .await;
        }
    }

    mod task_rescheduling {
        use std::future::Future;
        use std::pin::Pin;
        use std::time::Duration;

        use candid::Deserialize;
        use ic_stable_structures::{StableBTreeMap, StableCell, VectorMemory};
        use tokio::sync::Notify;

        use super::*;
        use crate::retry::BackoffPolicy;

        #[derive(Serialize, Deserialize, Debug, Clone)]
        struct SucceedingTask {}
        impl Task for SucceedingTask {
            type Ctx = ();

            fn execute(
                &self,
                _context: Self::Ctx,
                _task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
            ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
                Box::pin(async move { Ok(()) })
            }
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        struct FailingTask {}
        impl Task for FailingTask {
            type Ctx = ();

            fn execute(
                &self,
                _context: Self::Ctx,
                _task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
            ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
                Box::pin(async move { Err(SchedulerError::TaskExecutionFailed("".to_string())) })
            }
        }

        thread_local! {
            static COMPLETE: Arc<Notify> = Arc::new(Notify::new());
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        struct AwaitingTask {}

        impl AwaitingTask {
            fn complete(&self) {
                COMPLETE.with(|v| v.notify_one());
            }
        }

        impl Task for AwaitingTask {
            type Ctx = ();

            fn execute(
                &self,
                _context: Self::Ctx,
                _task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
            ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
                let complete = COMPLETE.with(|v| v.clone());
                Box::pin(async move {
                    println!("Waiting for execution");
                    complete.notified().await;
                    println!("Executing task after completion");
                    Err(SchedulerError::TaskExecutionFailed("".to_string()))
                })
            }
        }

        #[tokio::test]
        async fn options_updated_for_scheduled_tasks() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let scheduler = Scheduler::new(map, sequence);
                    let task = SucceedingTask {};

                    let id = scheduler.append_task((task, TaskOptions::new()).into());

                    let options = TaskOptions::new().with_max_retries_policy(5);
                    scheduler.reschedule(id, options.clone());

                    assert_eq!(scheduler.get_task(id).unwrap().options, options);
                })
                .await;
        }

        #[tokio::test]
        async fn options_updated_for_retrying_tasks() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let scheduler = Scheduler::new(map, sequence);
                    let task = FailingTask {};

                    let id = scheduler
                        .append_task((task, TaskOptions::new().with_max_retries_policy(3)).into());
                    scheduler.run(()).unwrap();
                    tokio::time::sleep(Duration::from_millis(25)).await;

                    let options = TaskOptions::new().with_max_retries_policy(5);
                    scheduler.reschedule(id, options.clone());

                    assert_eq!(scheduler.get_task(id).unwrap().options, options);
                })
                .await;
        }

        #[tokio::test]
        async fn reschedule_is_noop_for_non_existing_task() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let map: StableBTreeMap<u64, InnerScheduledTask<SucceedingTask>, _> =
                        StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let scheduler = Scheduler::new(map, sequence);

                    scheduler.reschedule(42, TaskOptions::new());
                })
                .await;
        }

        #[tokio::test]
        async fn options_updated_for_running_tasks() {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    let map = StableBTreeMap::new(VectorMemory::default());
                    let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
                    let scheduler = Scheduler::new(map, sequence);
                    let task = AwaitingTask {};

                    let id = scheduler.append_task(
                        (
                            task.clone(),
                            TaskOptions::new()
                                .with_max_retries_policy(3)
                                .with_backoff_policy(BackoffPolicy::None),
                        )
                            .into(),
                    );
                    scheduler.run(()).unwrap();
                    task.complete();
                    tokio::time::sleep(Duration::from_millis(25)).await;

                    dbg!(scheduler.get_task(id));

                    scheduler.run(()).unwrap();
                    let options = TaskOptions::new()
                        .with_execute_after_timestamp_in_secs(1000)
                        .with_max_retries_policy(5);
                    scheduler.reschedule(id, options.clone());

                    assert_eq!(scheduler.get_task(id).unwrap().options, options);

                    task.complete();
                    tokio::time::sleep(Duration::from_millis(25)).await;

                    assert_ne!(scheduler.get_task(id).unwrap().options, options);
                    assert_eq!(scheduler.get_task(id).unwrap().options.failures, 1);
                    assert_eq!(
                        scheduler.get_task(id).unwrap().options.retry_strategy,
                        options.retry_strategy
                    );
                })
                .await;
        }
    }

    mod test_find_id {
        use ic_stable_structures::{StableBTreeMap, StableCell, VectorMemory};

        use crate::scheduler::test::test_delay::SimpleTask;
        use crate::scheduler::{Scheduler, TaskScheduler};
        use crate::task::TaskOptions;

        #[test]
        fn finding_id_by_task_returns_correct_id() {
            let map = StableBTreeMap::new(VectorMemory::default());
            let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
            let scheduler = Scheduler::new(map, sequence);
            for id in 0..10 {
                let id_in_scheduler =
                    scheduler.append_task((SimpleTask::StepOne { id }, TaskOptions::new()).into());
                assert_eq!(id_in_scheduler, id);
            }

            const TO_FIND: u64 = 7;
            let found = scheduler
                .find_id(&|task| matches!(task, SimpleTask::StepOne { id } if id == TO_FIND));
            assert_eq!(found, Some(TO_FIND));
        }

        #[test]
        fn finding_id_by_task_returns_none_if_not_found() {
            let map = StableBTreeMap::new(VectorMemory::default());
            let sequence = StableCell::new(VectorMemory::default(), 0).unwrap();
            let scheduler = Scheduler::new(map, sequence);
            for id in 0..10 {
                let id_in_scheduler =
                    scheduler.append_task((SimpleTask::StepOne { id }, TaskOptions::new()).into());
                assert_eq!(id_in_scheduler, id);
            }

            const TO_FIND: u64 = 42;
            let found = scheduler
                .find_id(&|task| matches!(task, SimpleTask::StepOne { id } if id == TO_FIND));
            assert_eq!(found, None);
        }

        #[test]
        fn should_get_next_task_from_the_sequence() {
            let base_task_id = 12345;
            let map = StableBTreeMap::new(VectorMemory::default());
            let sequence = StableCell::new(VectorMemory::default(), base_task_id).unwrap();
            let scheduler = Scheduler::new(map, sequence);
            
            for id in base_task_id..(base_task_id + 10) {
                let id_in_scheduler =
                    scheduler.append_task((SimpleTask::StepOne { id }, TaskOptions::new()).into());
                assert_eq!(id_in_scheduler, id);
            }

            let to_find = base_task_id + 7;
            let found = scheduler
                .find_id(&|task| matches!(task, SimpleTask::StepOne { id } if id == to_find));
            assert_eq!(found, Some(to_find));
        }
    }
}
