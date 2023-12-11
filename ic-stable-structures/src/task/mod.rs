use std::{sync::Arc, pin::Pin};

use parking_lot::Mutex;

use crate::{VecStructure, Result};

/// A sync task is a unit of work that can be executed by the scheduler.
pub trait SyncTask {

    /// Execute the task and return the next task to execute.
    fn execute(&self) -> Option<Task>;
}

/// An async task is a unit of work that can be executed by the scheduler.
pub trait AsyncTask {

    /// Execute the task and return the next task to execute.
    fn execute(&self) -> Pin<Box<dyn std::future::Future<Output = Option<Task>>>>;
}

/// A task is a unit of work that can be executed by the scheduler.
pub enum Task {
    Sync(Box<dyn SyncTask>),
    Async(Box<dyn AsyncTask>),
}

impl From<Box<dyn SyncTask>> for Task {
    fn from(task: Box<dyn SyncTask>) -> Self {
        Self::Sync(task)
    }
}

impl From<Box<dyn AsyncTask>> for Task {
    fn from(task: Box<dyn AsyncTask>) -> Self {
        Self::Async(task)
    }    
}

/// A scheduler is responsible for executing tasks.
pub struct Scheduler<T: VecStructure<Task>> {
    pending_tasks: Arc<Mutex<T>>,
}

impl <T: VecStructure<Task>> Scheduler<T> {

    pub fn new(pending_tasks: T) -> Self {
        Self {
            pending_tasks: Arc::new(Mutex::new(pending_tasks)),
        }
    }

    /// Add a task to the scheduler.
    /// It will be executed at some point in the future when the Scheduler `run` function is executed.
    pub fn add_task(&mut self, task: Option<Task>) -> Result<()> {
        if let Some(task) = task {
            self.pending_tasks.lock().push(&task)
        } else {
            Ok(())
        }
    }

    /// Execute all pending tasks.
    pub fn run(&mut self) -> Result<()> {
        while let Some(task) = self.pending_tasks.lock().get(0) {
            match task {
                Task::Sync() => {
                    println!("Sync task");
                },
                Task::Async() => {
                    println!("Async task");
                },
            }
        }
        Ok(())
    }
}

fn execute_sync_task<T: VecStructure<Task>>(task: Box<dyn FnOnce() -> Option<Task>>, pending_tasks: Arc<Mutex<T>>) {
    if let Some(next_task) = task() {
        pending_tasks.lock().push(&next_task);
    }
}

fn execute_async_task2<T: 'static + VecStructure<Task>>(task: Box<dyn FnOnce() -> Pin<Box<dyn std::future::Future<Output = Option<Task>>>>>, pending_tasks: Arc<Mutex<T>>) {
    ic_cdk::spawn(async move {
        if let Some(next_task) = task().await {
            pending_tasks.lock().push(&next_task);
        }
    })
}
