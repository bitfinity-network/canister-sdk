use std::{sync::Arc, pin::Pin, future::Future};

use parking_lot::Mutex;

use crate::{Result, SlicedStorable, UnboundedMapStructure};

/// A sync task is a unit of work that can be executed by the scheduler.
pub trait Task: SlicedStorable {
    /// Execute the task and return the next task to execute.
    fn execute(&self, task_scheduler: Box<dyn 'static + TaskScheduler<Self>>) -> Pin<Box<dyn Future<Output = Result<()>>>>;
}

/// A scheduler is responsible for executing tasks.
#[derive(Clone)]
pub struct Scheduler<T: 'static + Task, P: 'static + UnboundedMapStructure<u32, T>> {
    pending_tasks: Arc<Mutex<P>>,
    phantom: std::marker::PhantomData<T>,
}

impl <T: 'static + Task, P: 'static + UnboundedMapStructure<u32, T>> Scheduler<T, P> {

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
                task.execute(task_scheduler).await?;
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
    fn append_task(&self, task: T);
}

impl <T: 'static + Task, P: 'static + UnboundedMapStructure<u32, T>> TaskScheduler<T> for Scheduler<T, P> {
    fn append_task(&self, task: T) {
        //self.pending_tasks.lock().push(&task)
        // this is O(n), but we can remove only using `pop`, so, if we push here, the last inserted task will be executed first
        let mut lock = self.pending_tasks.lock();
        let key = lock.last_key().map(|val| val + 1).unwrap_or_default();
        lock.insert(&key, &task);
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
                    task_scheduler.append_task(TestTask::StepTwo);
                    Ok(())
                }),
                TestTask::StepTwo => Box::pin(async move {
                    println!("StepTwo");

                    // More tasks can be appended to the scheduler. BEWARE of circular dependencies!!
                    task_scheduler.append_task(TestTask::StepThree);
                    task_scheduler.append_task(TestTask::StepThree);
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
        
        scheduler.append_task(TestTask::StepOne);
        scheduler.run().await.unwrap();
    }

}