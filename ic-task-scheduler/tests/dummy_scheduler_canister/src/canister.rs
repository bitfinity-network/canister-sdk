use std::{cell::RefCell, future::Future, pin::Pin, sync::Arc, time::Duration};

use candid::Principal;
use ic_canister::{generate_idl, init, query, Canister, Idl, PreUpdate};
use ic_exports::ic_kit::RejectionCode;
use ic_stable_structures::{
    default_ic_memory_manager, stable_structures::DefaultMemoryImpl, IcMemoryManager, MemoryId,
    StableUnboundedMap, VirtualMemory,
};
use ic_task_scheduler::{
    scheduler::{Scheduler, TaskScheduler},
    task::{ScheduledTask, Task, TaskOptions},
    SchedulerError,
};
use serde::{Deserialize, Serialize};

type Storage = StableUnboundedMap<u32, ScheduledTask<PanicTask>, VirtualMemory<DefaultMemoryImpl>>;
type PanickingScheduler = Scheduler<PanicTask, Storage, DefaultMemoryImpl>;

const SCHEDULER_STORAGE_MEMORY_ID: MemoryId = MemoryId::new(1);
const TASK_QUEUE_MEMORY_ID: MemoryId = MemoryId::new(2);

thread_local! {
    pub static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());

    static SCHEDULER: RefCell<Arc<PanickingScheduler>> = {
        let mut map: Storage = Storage::new(MEMORY_MANAGER.with(|mm| mm.get(SCHEDULER_STORAGE_MEMORY_ID)));

        let memory_manager = default_ic_memory_manager();

        let mut scheduler = PanickingScheduler::new(
            map,
            &memory_manager,
            TASK_QUEUE_MEMORY_ID
        ).unwrap();
        scheduler.set_failed_task_callback(move |_, _| {
            FAILED_TASK_CALLED.with_borrow_mut(|called| {
                *called = true;
            });
        });
        scheduler.set_save_state_query_callback(Box::new(save_state_cb));
        scheduler.append_task(
            (
                PanicTask::StepOne { id: 1 },
                TaskOptions::new()
                    .with_max_retries_policy(0)
                    .with_fixed_backoff_policy(0),
            )
                .into(),
        );
        RefCell::new(Arc::new(scheduler))
    };

    static SAVE_STATE_CALLED: RefCell<bool> = RefCell::new(false);

    static FAILED_TASK_CALLED : RefCell<bool> = RefCell::new(false);

    static PRINCIPAL : RefCell<Principal> = RefCell::new(Principal::anonymous());

}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PanicTask {
    StepOne { id: u32 },
}

impl Task for PanicTask {
    fn execute(
        &self,
        _task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
        panic!("PanicTask::execute")
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
        ic_exports::ic_cdk_timers::set_timer(Duration::from_millis(10), || {
            ic_cdk::spawn(Self::run_scheduler())
        });

        // set principal
        PRINCIPAL.with_borrow_mut(|principal| {
            *principal = self.id;
        });
    }

    #[query]
    pub fn save_state(&self) -> bool {
        SAVE_STATE_CALLED.with_borrow_mut(|called| {
            *called = true;
        });

        true
    }

    #[query]
    pub fn save_state_called(&self) -> bool {
        SAVE_STATE_CALLED.with_borrow(|called| *called)
    }

    #[query]
    pub fn failed_task_called(&self) -> bool {
        FAILED_TASK_CALLED.with_borrow(|called| *called)
    }

    async fn run_scheduler() {
        let scheduler = SCHEDULER.with_borrow_mut(|scheduler| scheduler.clone());
        scheduler.run().await.unwrap();
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}

async fn save_state() -> Result<(), (RejectionCode, String)> {
    let canister = PRINCIPAL.with_borrow(|principal| *principal);
    ic_exports::ic_cdk::call(canister, "save_state", ()).await
}

fn save_state_cb() -> Pin<Box<dyn Future<Output = Result<(), (RejectionCode, String)>>>> {
    Box::pin(async { save_state().await })
}
