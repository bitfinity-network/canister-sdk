mod scheduler;
mod wasm_utils;

use std::{future::Future, pin::Pin, time::Duration};

use candid::{CandidType, Encode, Principal};
use ic_canister_client::PocketIcClient;
use ic_exports::pocket_ic::nio::PocketIcAsync;
use ic_kit::mock_principals::alice;
use ic_task_scheduler::{scheduler::TaskScheduler, task::{InnerScheduledTask, Task}, SchedulerError};
use serde::{Deserialize, Serialize};
use wasm_utils::get_dummy_scheduler_canister_bytecode;

#[derive(Clone)]
pub struct PocketIcTestContext {
    canister_client: PocketIcClient,
    client: PocketIcAsync,
    pub dummy_scheduler_canister: Principal,
}

impl PocketIcTestContext {

    /// Returns the PocketIC client for the canister.
    pub fn client(&self) -> &PocketIcAsync {
        &self.client
    }

    pub async fn get_task(&self, task_id: u32) -> Option<InnerScheduledTask<DummyTask>> {
        self.canister_client.query("get_task", (task_id, )).await.unwrap()
    }

    pub async fn completed_tasks(&self) -> Vec<u32> {
        self.canister_client.query("completed_tasks", ()).await.unwrap()
    }

    pub async fn panicked_tasks(&self) -> Vec<u32> {
        self.canister_client.query("panicked_tasks", ()).await.unwrap()
    }

    pub async fn failed_tasks(&self) -> Vec<u32> {
        self.canister_client.query("failed_tasks", ()).await.unwrap()
    }

    pub async fn schedule_tasks(&self, tasks: Vec<DummyTask>) -> Vec<u32> {
        self.canister_client.update("schedule_tasks", (tasks, )).await.unwrap()
    }

    pub async fn run_scheduler(&self) {
        self.client.advance_time(Duration::from_millis(5000)).await;
        self.client.tick().await;
    }

}

async fn deploy_dummy_scheduler_canister() -> anyhow::Result<PocketIcTestContext> {
    let client = PocketIcAsync::init().await;
    println!("Creating dummy canister");
    
    let sender = alice();
    let canister = client.create_canister(Some(sender)).await;
    println!("Canister created with principal {}", canister);
    
    let canister_client = ic_canister_client::PocketIcClient::from_client(client.clone(), canister, alice());
    
    let env = PocketIcTestContext {
        canister_client,
        client,
        dummy_scheduler_canister: canister,
    };
    
    env.client().add_cycles(canister, 10_u128.pow(14)).await;
    println!("cycles added");
    
    let dummy_wasm = get_dummy_scheduler_canister_bytecode();
    let args = Encode!(&())?;
    env.client()
        .install_canister(canister, dummy_wasm.to_vec(), args, Some(sender))
        .await;

    println!("Installed dummy canister");

    Ok(env)
}


#[derive(CandidType, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
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
        Box::pin(async move { Ok(()) })
    }
}