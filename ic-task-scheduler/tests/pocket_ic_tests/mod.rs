mod scheduler;
mod wasm_utils;

use std::time::Duration;

use candid::{CandidType, Decode, Encode, Principal};
use ic_exports::ic_kit::ic;
use ic_exports::pocket_ic;
use ic_exports::pocket_ic::nio::PocketIcAsync;
use ic_kit::mock_principals::alice;
use pocket_ic::WasmResult;
use serde::Deserialize;
use wasm_utils::get_dummy_scheduler_canister_bytecode;

#[derive(Clone)]
pub struct PocketIcTestContext {
    client: PocketIcAsync,
    pub dummy_scheduler_canister: Principal,
}

impl PocketIcTestContext {
    /// Returns the PocketIC client for the canister.
    pub fn client(&self) -> &PocketIcAsync {
        &self.client
    }

    async fn query_as<Result>(
        &self,
        sender: Principal,
        canister_id: Principal,
        method: &str,
        payload: Vec<u8>,
    ) -> Result
    where
        for<'a> Result: CandidType + Deserialize<'a>,
    {
        let res = match self
            .client
            .query_call(canister_id, sender, method.to_string(), payload)
            .await
            .unwrap()
        {
            WasmResult::Reply(bytes) => bytes,
            WasmResult::Reject(e) => panic!("Unexpected reject: {:?}", e),
        };

        Decode!(&res, Result).expect("failed to decode item from candid")
    }

    pub async fn scheduled_state_called(&self) -> bool {
        let args = Encode!(&()).unwrap();
        let res = self
            .query_as(
                ic::caller(),
                self.dummy_scheduler_canister,
                "scheduled_state_called",
                args,
            )
            .await;

        res
    }

    pub async fn completed_tasks(&self) -> Vec<u32> {
        let args = Encode!(&()).unwrap();
        let res = self
            .query_as(
                ic::caller(),
                self.dummy_scheduler_canister,
                "completed_tasks",
                args,
            )
            .await;

        res
    }

    pub async fn panicked_tasks(&self) -> Vec<u32> {
        let args = Encode!(&()).unwrap();
        let res = self
            .query_as(
                ic::caller(),
                self.dummy_scheduler_canister,
                "panicked_tasks",
                args,
            )
            .await;

        res
    }

    pub async fn failed_tasks(&self) -> Vec<u32> {
        let args = Encode!(&()).unwrap();
        let res = self
            .query_as(
                ic::caller(),
                self.dummy_scheduler_canister,
                "failed_tasks",
                args,
            )
            .await;

        res
    }

    pub async fn executed_tasks(&self) -> Vec<u32> {
        let args = Encode!(&()).unwrap();
        let res = self
            .query_as(
                ic::caller(),
                self.dummy_scheduler_canister,
                "executed_tasks",
                args,
            )
            .await;

        res
    }

    pub async fn run_scheduler(&self) {
        self.client.advance_time(Duration::from_millis(5000)).await;
        self.client.tick().await;
    }
}

async fn deploy_dummy_scheduler_canister() -> anyhow::Result<PocketIcTestContext> {
    let client = PocketIcAsync::init().await;
    let dummy_wasm = get_dummy_scheduler_canister_bytecode();
    println!("Creating dummy canister");

    let args = Encode!(&())?;

    let sender = alice();
    let canister = client.create_canister(Some(sender)).await;
    println!("Canister created with principal {}", canister);
    let env = PocketIcTestContext {
        client,
        dummy_scheduler_canister: canister,
    };

    env.client().add_cycles(canister, 10_u128.pow(12)).await;
    println!("cycles added");
    env.client()
        .install_canister(canister, dummy_wasm.to_vec(), args, Some(sender))
        .await;

    println!("Installed dummy canister");

    Ok(env)
}
