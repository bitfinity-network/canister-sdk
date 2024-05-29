use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use candid::{CandidType, Principal};
use ic_canister::{generate_idl, init, post_upgrade, query, update, Canister, Idl, PreUpdate};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{IcMemoryManager, MemoryId, StableBTreeMap, VirtualMemory};
use ic_task_scheduler::scheduler::{Scheduler, TaskScheduler};
use ic_task_scheduler::task::{InnerScheduledTask, ScheduledTask, Task, TaskStatus};
use ic_task_scheduler::SchedulerError;
use serde::{Deserialize, Serialize};

type Storage =
    StableBTreeMap<u32, InnerScheduledTask<DummyTask>, VirtualMemory<DefaultMemoryImpl>>;
type PanickingScheduler = Scheduler<DummyTask, Storage>;

const SCHEDULER_STORAGE_MEMORY_ID: MemoryId = MemoryId::new(1);

thread_local! {
    pub static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());

    static SCHEDULER: RefCell<PanickingScheduler> = {
        let map: Storage = Storage::new(MEMORY_MANAGER.with(|mm| mm.get(SCHEDULER_STORAGE_MEMORY_ID)));

        let mut scheduler = PanickingScheduler::new(
            map,
        );

        scheduler.set_running_task_timeout(30);
        scheduler.on_completion_callback(save_state_cb);

        RefCell::new(scheduler)
    };

    static COMPLETED_TASKS: RefCell<Vec<u32>> = const { RefCell::new(vec![]) };
    static FAILED_TASKS: RefCell<Vec<u32>> = const { RefCell::new(vec![]) };
    static PANICKED_TASKS : RefCell<Vec<u32>> = const { RefCell::new(vec![]) };

    static PRINCIPAL : RefCell<Principal> = const { RefCell::new(Principal::anonymous()) };

}

#[derive(CandidType, Serialize, Deserialize, Debug, Clone)]
pub enum DummyTask {
    Panicking,
    GoodTask,
    FailTask,
}

impl Task for DummyTask {
    fn execute(
        &self,
        _task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
        match self {
            Self::GoodTask => Box::pin(async move { Ok(()) }),
            Self::Panicking => Box::pin(async move {
                panic!("PanicTask::execute");
            }),
            Self::FailTask => Box::pin(async move {
                Err(SchedulerError::TaskExecutionFailed(
                    "i dunno why".to_string(),
                ))
            }),
        }
    }
}

#[derive(Canister)]
pub struct DummyCanister {
    #[id]
    id: Principal,
}

impl PreUpdate for DummyCanister {}

impl DummyCanister {
    #[init]
    pub fn init(&self) {
        self.set_timers();

        // set principal
        PRINCIPAL.with_borrow_mut(|principal| {
            *principal = self.id;
        });
    }

    #[post_upgrade]
    pub fn post_upgrade(&self) {
        self.set_timers();
    }

    fn set_timers(&self) {
        ic_exports::ic_cdk_timers::set_timer_interval(Duration::from_millis(10), || {
            Self::do_run_scheduler()
        });
    }

    #[query]
    pub fn panicked_tasks(&self) -> Vec<u32> {
        PANICKED_TASKS.with_borrow(|tasks| tasks.clone())
    }

    #[query]
    pub fn completed_tasks(&self) -> Vec<u32> {
        COMPLETED_TASKS.with_borrow(|tasks| tasks.clone())
    }

    #[query]
    pub fn failed_tasks(&self) -> Vec<u32> {
        FAILED_TASKS.with_borrow(|tasks| tasks.clone())
    }

    #[query]
    pub fn get_task(&self, task_id: u32) -> Option<InnerScheduledTask<DummyTask>> {
        let scheduler = SCHEDULER.with_borrow(|scheduler| scheduler.clone());
        scheduler.get_task(task_id)
    }

    #[update]
    pub fn schedule_tasks(&self, tasks: Vec<DummyTask>) -> Vec<u32> {
        let scheduler = SCHEDULER.with_borrow(|scheduler| scheduler.clone());
        let scheduled_tasks = tasks.into_iter().map(ScheduledTask::new).collect();
        scheduler.append_tasks(scheduled_tasks)
    }

    #[update]
    pub fn run_scheduler(&self) {
        Self::do_run_scheduler();
    }

    fn do_run_scheduler() {
        let scheduler = SCHEDULER.with_borrow(|scheduler| scheduler.clone());
        scheduler.run().unwrap();
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}

fn save_state_cb(task: InnerScheduledTask<DummyTask>) {
    match task.status() {
        TaskStatus::Waiting { .. } => {}
        TaskStatus::Completed { .. } => {
            COMPLETED_TASKS.with_borrow_mut(|tasks| {
                tasks.push(task.id());
            });
        }
        TaskStatus::Running { .. } => {}
        TaskStatus::Failed { .. } => {
            FAILED_TASKS.with_borrow_mut(|tasks| {
                tasks.push(task.id());
            });
        }
        TaskStatus::TimeoutOrPanic { .. } => {
            PANICKED_TASKS.with_borrow_mut(|tasks| {
                tasks.push(task.id());
            });
        }
        TaskStatus::Scheduled { .. } => {}
    };
}
