use std::sync::Mutex;

use anyhow::Result;
use candid::{Principal, CandidType, Deserialize};
use candid::{Decode, Encode};
use did::Transaction;
use ic_exports::ic_kit::ic;
use ic_exports::ic_kit::inject;
use ic_exports::ic_kit::mock_principals::alice;
use ic_exports::ic_test_state_machine::{StateMachine, get_ic_test_state_machine_client_path, WasmResult};
use once_cell::sync::Lazy;

use crate::utils::wasm::get_dummy_canister_bytecode;

mod btreemap;
mod cell;
mod log;
mod map;
mod multimap;
mod ring_buffer;
mod vec;

pub struct StateMachineTestContext {
    pub env: StateMachine,
    pub dummy_canister: Principal,
}

impl StateMachineTestContext {

    pub fn update_call_as<Result>(
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
    
    pub fn get_tx_from_btreemap(&self, key: u64) -> Result<Option<Transaction>> {
        let args = Encode!(&key).unwrap();
        let res = self.update_call_as(
            ic::caller(),
            self.dummy_canister,
            "get_tx_from_btreemap",
            args,
        );

        Ok(res)
    }

    pub fn insert_tx_to_btreemap(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        let res = self.update_call_as(
            ic::caller().into(),
            self.dummy_canister,
            "insert_tx_to_btreemap",
            args,
        );

        Ok(res)
    }

    pub fn get_tx_from_cell(&self) -> Result<Transaction> {
        let args = Encode!(&()).unwrap();
        let res = self.update_call_as(
            ic::caller().into(),
            self.dummy_canister,
            "get_tx_from_cell",
            args,
        );

        Ok(res)
    }

    pub fn insert_tx_to_cell(&self, from: u8, to: u8, value: u8) -> Result<()> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        let res = self.update_call_as(
            ic::caller().into(),
            self.dummy_canister,
            "insert_tx_to_cell",
            args,
        );

        Ok(res)
    }

    pub fn get_tx_from_map(&self, key: u64) -> Result<Option<Transaction>> {
        let args = Encode!(&key).unwrap();
        let res = self.update_call_as(
            ic::caller().into(),
            self.dummy_canister,
            "get_tx_from_map",
            args,
        );

        Ok(res)
    }

    pub fn insert_tx_to_map(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        let res = self.update_call_as(
            ic::caller().into(),
            self.dummy_canister,
            "insert_tx_to_map",
            args,
        );

        Ok(res)
    }

    pub fn get_tx_from_multimap(&self, key: u64) -> Result<Option<Transaction>> {
        let args = Encode!(&key).unwrap();
        let res = self.update_call_as(
            ic::caller().into(),
            self.dummy_canister,
            "get_tx_from_multimap",
            args,
        );

        Ok(res)
    }

    pub fn insert_tx_to_multimap(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        let res = self.update_call_as(
            ic::caller().into(),
            self.dummy_canister,
            "insert_tx_to_multimap",
            args,
        );

        Ok(res)
    }

    pub fn get_tx_from_vec(&self, index: u64) -> Result<Option<Transaction>> {
        let args = Encode!(&index).unwrap();
        let res = self.update_call_as(
            ic::caller().into(),
            self.dummy_canister,
            "get_tx_from_vec",
            args,
        );

        Ok(res)
    }

    pub fn push_tx_to_vec(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        let res = self.update_call_as(
            ic::caller().into(),
            self.dummy_canister,
            "push_tx_to_vec",
            args,
        );

        Ok(res)
    }

    pub fn get_tx_from_ring_buffer(&self, index: u64) -> Result<Option<Transaction>> {
        let args = Encode!(&index).unwrap();
        let res = self.update_call_as(
            ic::caller().into(),
            self.dummy_canister,
            "get_tx_from_ring_buffer",
            args,
        );

        Ok(res)
    }

    pub fn push_tx_to_ring_buffer(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        let res = self.update_call_as(
            ic::caller().into(),
            self.dummy_canister,
            "push_tx_to_ring_buffer",
            args,
        );

        Ok(res)
    }

    pub fn get_tx_from_log(&self, index: u64) -> Result<Option<Transaction>> {
        let args = Encode!(&index).unwrap();
        let res = self.update_call_as(
            ic::caller().into(),
            self.dummy_canister,
            "get_tx_from_log",
            args,
        );

        Ok(res)
    }

    pub fn push_tx_to_log(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        let res = self.update_call_as(
            ic::caller().into(),
            self.dummy_canister,
            "push_tx_to_log",
            args,
        );

        Ok(res)
    }
}

pub fn with_state_machine_context<'a, F>(f: F) -> Result<()>
where
    F: FnOnce(&'a mut ic_exports::ic_kit::MockContext, &StateMachineTestContext) -> Result<()>,
{
    ic_exports::ic_kit::MockContext::new()
        .with_caller(alice())
        .with_id(alice())
        .inject();

    static TEST_CONTEXT: Lazy<Mutex<StateMachineTestContext>> = Lazy::new(|| {
        let client_path = get_ic_test_state_machine_client_path("../target");
        let env = StateMachine::new(&client_path, false);
        let dummy_canister = deploy_dummy_canister(&env).unwrap();
        StateMachineTestContext {
            env,
            dummy_canister,
        }
        .into()
    });
    let test_ctx = TEST_CONTEXT.lock().unwrap();
    let ctx = inject::get_context();

    f(ctx, &test_ctx)?;

    reinstall_dummy_canister(&test_ctx)?;

    Ok(())
}

fn deploy_dummy_canister(env: &StateMachine) -> Result<Principal> {
    let dummy_wasm = get_dummy_canister_bytecode();
    eprintln!("Creating dummy canister");

    let args = Encode!(&())?;

    let canister = env.create_canister(None);
    env.add_cycles(canister, 10_u128.pow(12));
    env.install_canister(canister, dummy_wasm.to_vec(), args, None);

    Ok(canister)
}

pub fn reinstall_dummy_canister(ctx: &StateMachineTestContext) -> Result<()> {
    let args = Encode!(&())?;

    let dummy_wasm = get_dummy_canister_bytecode();

    ctx.env
        .reinstall_canister(ctx.dummy_canister, dummy_wasm, args, None).unwrap();

    Ok(())
}

pub fn upgrade_dummy_canister(ctx: &StateMachineTestContext) -> Result<()> {
    let args = Encode!(&())?;

    let dummy_wasm = get_dummy_canister_bytecode();

    ctx.env
        .upgrade_canister(ctx.dummy_canister, dummy_wasm, args, None).unwrap();

    Ok(())
}
