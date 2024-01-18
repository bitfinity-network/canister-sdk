use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicU8;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use ic_exports::pocket_ic;
use ic_kit::mock_principals::alice;
use ic_kit::{inject, RejectionCode};
use ic_stable_structures::{default_ic_memory_manager, StableUnboundedMap, VectorMemory};
use ic_task_scheduler::scheduler::{Scheduler, TaskScheduler};
use ic_task_scheduler::task::{ScheduledTask, Task, TaskOptions};
use ic_task_scheduler::SchedulerError;
use serde::{Deserialize, Serialize};

use super::PocketIcTestContext;
use crate::pocket_ic_tests::deploy_dummy_scheduler_canister;

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

#[tokio::test]
async fn test_should_remove_panicking_task() {
    ic_exports::ic_kit::MockContext::new()
        .with_caller(alice())
        .with_id(alice())
        .inject();

    let env = pocket_ic::init_pocket_ic();
    let dummy_scheduler_canister = deploy_dummy_scheduler_canister(&env).unwrap();

    let test_ctx = PocketIcTestContext {
        env,
        dummy_scheduler_canister,
    };

    let ctx = inject::get_context();

    let map: StableUnboundedMap<
        u32,
        ScheduledTask<PanicTask>,
        std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
    > = StableUnboundedMap::new(VectorMemory::default());
    let memory_manager: ic_stable_structures::IcMemoryManager<
        std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
    > = default_ic_memory_manager();

    // set error callback
    let called = Arc::new(AtomicU8::new(0));
    let called_t = called.clone();

    let mut scheduler = Scheduler::new(map, &memory_manager, 1).unwrap();
    scheduler.set_failed_task_callback(move |_, _| {
        called_t.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    let save_state_cb = Box::pin(|| async_function());
    scheduler.set_save_state_query_callback(save_state_cb);

    let id = 1;
    scheduler.append_task(
        (
            PanicTask::StepOne { id },
            TaskOptions::new()
                .with_max_retries_policy(0)
                .with_fixed_backoff_policy(0),
        )
            .into(),
    );

    // After the last retries the task is removed
    scheduler.run().await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    assert_eq!(called.load(std::sync::atomic::Ordering::SeqCst), 1);
}

async fn async_function() -> Result<(), RejectionCode> {
    Ok(())
}
