mod scheduler;
mod wasm_utils;

use anyhow::Result;
use candid::{CandidType, Decode, Encode, Principal};
use ic_exports::ic_kit::{ic, inject};
use ic_exports::pocket_ic;
use ic_kit::mock_principals::alice;
use pocket_ic::{PocketIc, WasmResult};
use serde::Deserialize;
use wasm_utils::get_dummy_scheduler_canister_bytecode;

pub struct PocketIcTestContext {
    pub env: PocketIc,
    pub dummy_scheduler_canister: Principal,
}

impl PocketIcTestContext {
    fn query_as<Result>(
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
            .env
            .query_call(canister_id, sender, method, payload)
            .unwrap()
        {
            WasmResult::Reply(bytes) => bytes,
            WasmResult::Reject(e) => panic!("Unexpected reject: {:?}", e),
        };

        Decode!(&res, Result).expect("failed to decode item from candid")
    }

    fn update_call_as<Result>(
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
            .env
            .update_call(canister_id, sender, method, payload)
            .unwrap()
        {
            WasmResult::Reply(bytes) => bytes,
            WasmResult::Reject(e) => panic!("Unexpected reject: {:?}", e),
        };

        Decode!(&res, Result).expect("failed to decode item from candid")
    }

    pub fn save_state(&self) -> Result<bool> {
        let args = Encode!(&()).unwrap();
        let res = self.query_as(
            ic::caller(),
            self.dummy_scheduler_canister,
            "save_state",
            args,
        );

        Ok(res)
    }
}

pub fn with_pocket_ic_context<'a, F>(f: F) -> Result<()>
where
    F: FnOnce(&'a mut ic_exports::ic_kit::MockContext, &PocketIcTestContext) -> Result<()>,
{
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

    f(ctx, &test_ctx)?;

    Ok(())
}

fn deploy_dummy_scheduler_canister(env: &PocketIc) -> Result<Principal> {
    let dummy_wasm = get_dummy_scheduler_canister_bytecode();
    eprintln!("Creating dummy canister");

    let args = Encode!(&())?;

    let canister = env.create_canister();
    env.add_cycles(canister, 10_u128.pow(12));
    env.install_canister(canister, dummy_wasm.to_vec(), args, None);

    Ok(canister)
}
