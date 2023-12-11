use std::sync::Arc;

use dfinity_stable_structures::Storable;
use parking_lot::Mutex;

use crate::{VecStructure, Result};

/// A sync task is a unit of work that can be executed by the scheduler.
pub trait Task: Storable{
    /// Execute the task and return the next task to execute.
    fn execute(&self, task_scheduler: Box<dyn 'static + TaskScheduler<Self>>) -> Result<()>;
}

/// A scheduler is responsible for executing tasks.
#[derive(Clone)]
pub struct Scheduler<T: 'static + Task, P: 'static + VecStructure<T>> {
    pending_tasks: Arc<Mutex<P>>,
    phantom: std::marker::PhantomData<T>,
}

impl <T: 'static + Task, P: 'static + VecStructure<T>> Scheduler<T, P> {

    pub fn new(pending_tasks: P) -> Self {
        Self {
            pending_tasks: Arc::new(Mutex::new(pending_tasks)),
            phantom: std::marker::PhantomData,
        }
    }

    /// Execute all pending tasks.
    pub fn run(&self) -> Result<()> {
        while let Some(task) = self.pending_tasks.lock().pop() {
            let task_scheduler = Box::new(Self {
                pending_tasks: self.pending_tasks.clone(),
                phantom: std::marker::PhantomData,
            });
            task.execute(task_scheduler)?;
        }
        Ok(())
    }
}

pub trait SchedulerExecutor {
    fn execute(&self);
}

pub trait TaskScheduler<T: 'static + Task> {
    fn append_task(&self, task: T) -> Result<()>;
}

impl <T: 'static + Task, P: 'static + VecStructure<T>> TaskScheduler<T> for Scheduler<T, P> {
    fn append_task(&self, task: T) -> Result<()> {
        //self.pending_tasks.lock().push(&task)
        // this is O(n), but we can remove only using `pop`, so, if we push here, the last inserted task will be executed first
        self.pending_tasks.lock().set(0, &task)
    }
}

#[cfg(test)] 
mod test {

    use dfinity_stable_structures::{Storable, VectorMemory, DefaultMemoryImpl};
    use ic_exports::ic_kit::MockContext;

    use crate::StableVec;
    use super::*;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
    pub enum TestTask {
        StepOne,
        StepTwo,
        StepThree,
    }

    impl Task for TestTask {
        fn execute(&self, task_scheduler: Box<dyn 'static + TaskScheduler<Self>>) -> Result<()> {
            match self {
                TestTask::StepOne => {
                    println!("StepOne");
                    task_scheduler.append_task(TestTask::StepTwo)?;
                },
                TestTask::StepTwo => {
                    println!("StepTwo");
                    ic_cdk::spawn(async move {
                        println!("Spawned task from StepTwo");
                        task_scheduler.append_task(TestTask::StepThree).unwrap();
                    });
                },
                TestTask::StepThree => {
                    println!("StepThree");
                },
            }
            Ok(())
        }
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

    #[test]
    fn test_spawn() {
        MockContext::new().inject();
        let vec = StableVec::<TestTask, _>::new(VectorMemory::default()).unwrap();
        let scheduler = Scheduler::new(vec);
        
        scheduler.append_task(TestTask::StepOne).unwrap();
        scheduler.run().unwrap();
    }

}