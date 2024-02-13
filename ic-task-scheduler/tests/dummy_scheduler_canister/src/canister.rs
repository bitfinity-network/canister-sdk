use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use candid::Principal;
use ic_canister::{generate_idl, init, post_upgrade, query, update, Canister, Idl, PreUpdate};
use ic_exports::ic_kit::RejectionCode;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{IcMemoryManager, MemoryId, StableUnboundedMap, VirtualMemory};
use ic_task_scheduler::scheduler::{Scheduler, TaskExecutionState, TaskScheduler};
use ic_task_scheduler::task::{ScheduledTask, Task, TaskOptions};
use ic_task_scheduler::SchedulerError;
use serde::{Deserialize, Serialize};

type Storage = StableUnboundedMap<u32, ScheduledTask<DummyTask>, VirtualMemory<DefaultMemoryImpl>>;
type PanickingScheduler = Scheduler<DummyTask, Storage>;

const SCHEDULER_STORAGE_MEMORY_ID: MemoryId = MemoryId::new(1);

thread_local! {
    pub static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());

    static SCHEDULER: RefCell<PanickingScheduler> = {
        let map: Storage = Storage::new(MEMORY_MANAGER.with(|mm| mm.get(SCHEDULER_STORAGE_MEMORY_ID)));

        let scheduler = PanickingScheduler::new(
            map,
            Some(Box::new(save_state_cb))
        ).unwrap();

        scheduler.append_task((DummyTask::GoodTask, TaskOptions::new()).into());
        scheduler.append_task((DummyTask::Panicking, TaskOptions::new()).into());
        scheduler.append_task((DummyTask::GoodTask, TaskOptions::new()).into());
        scheduler.append_task((DummyTask::FailTask, TaskOptions::new()).into());

        RefCell::new(scheduler)
    };

    static SCHEDULED_STATE_CALLED: RefCell<bool> = RefCell::new(false);
    static COMPLETED_TASKS: RefCell<Vec<u32>> = RefCell::new(vec![]);
    static FAILED_TASKS: RefCell<Vec<u32>> = RefCell::new(vec![]);
    static PANICKED_TASKS : RefCell<Vec<u32>> = RefCell::new(vec![]);
    static EXECUTING_TASKS : RefCell<Vec<u32>> = RefCell::new(vec![]);

    static PRINCIPAL : RefCell<Principal> = RefCell::new(Principal::anonymous());

}

#[derive(Serialize, Deserialize, Debug, Clone)]
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
            ic_cdk::spawn(Self::do_run_scheduler())
        });
    }

    #[update]
    pub fn save_state(&self) -> bool {
        SCHEDULED_STATE_CALLED.with_borrow_mut(|called| {
            *called = true;
        });

        true
    }

    #[query]
    pub fn scheduled_state_called(&self) -> bool {
        SCHEDULED_STATE_CALLED.with_borrow(|called| *called)
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
    pub fn executed_tasks(&self) -> Vec<u32> {
        EXECUTING_TASKS.with_borrow(|tasks| tasks.clone())
    }

    #[update]
    pub async fn run_scheduler(&self) {
        Self::do_run_scheduler().await
    }

    async fn do_run_scheduler() {
        let mut scheduler = SCHEDULER.with_borrow(|scheduler| scheduler.clone());
        scheduler.run().await.unwrap();
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}

async fn save_state(state: TaskExecutionState) -> Result<(), (RejectionCode, String)> {
    let canister = PRINCIPAL.with_borrow(|principal| *principal);
    match state {
        TaskExecutionState::Completed(id) => {
            COMPLETED_TASKS.with_borrow_mut(|tasks| {
                tasks.push(id);
            });
        }
        TaskExecutionState::Panicked(id) => {
            PANICKED_TASKS.with_borrow_mut(|tasks| {
                tasks.push(id);
            });
        }
        TaskExecutionState::Failed(id, _) => {
            FAILED_TASKS.with_borrow_mut(|tasks| {
                tasks.push(id);
            });
        }
        TaskExecutionState::Executing(id) => {
            EXECUTING_TASKS.with_borrow_mut(|tasks| {
                tasks.push(id);
            });
        }
        TaskExecutionState::Scheduled => {}
    }
    ic_exports::ic_cdk::call(canister, "save_state", ()).await
}

type SaveStateCb = Pin<Box<dyn Future<Output = Result<(), (RejectionCode, String)>>>>;

fn save_state_cb(state: TaskExecutionState) -> SaveStateCb {
    Box::pin(async { save_state(state).await })
}
