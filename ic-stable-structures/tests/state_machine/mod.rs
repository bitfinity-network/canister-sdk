use std::sync::Mutex;

use anyhow::Result;
use candid::{Decode, Encode};
use did::Transaction;
use ic_exports::ic_kit::ic;
use ic_exports::ic_kit::inject;
use ic_exports::ic_kit::mock_principals::alice;
use ic_exports::ic_state_machine_tests::{CanisterId, Cycles, StateMachine};
use once_cell::sync::Lazy;

use crate::utils::wasm::get_dummy_canister_bytecode;

mod btreemap;
mod cell;
mod log;
mod map;
mod multimap;
mod ring_buffer;
mod vec;

#[derive(Debug)]
pub struct StateMachineTestContext {
    pub env: StateMachine,
    pub dummy_canister: CanisterId,
}

impl StateMachineTestContext {
    pub fn get_tx_from_btreemap(&self, key: u64) -> Result<Option<Transaction>> {
        let args = Encode!(&key).unwrap();
        let res = self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "get_tx_from_btreemap",
            args,
        )?;

        let tx = Decode!(&res.bytes(), Option<Transaction>)?;

        Ok(tx)
    }

    pub fn insert_tx_to_btreemap(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        let res = self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "insert_tx_to_btreemap",
            args,
        )?;

        let key = Decode!(&res.bytes(), u64)?;

        Ok(key)
    }

    pub fn get_tx_from_cell(&self) -> Result<Transaction> {
        let args = Encode!(&()).unwrap();
        let res = self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "get_tx_from_cell",
            args,
        )?;

        let tx = Decode!(&res.bytes(), Transaction)?;

        Ok(tx)
    }

    pub fn insert_tx_to_cell(&self, from: u8, to: u8, value: u8) -> Result<()> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "insert_tx_to_cell",
            args,
        )?;

        Ok(())
    }

    pub fn get_tx_from_map(&self, key: u64) -> Result<Option<Transaction>> {
        let args = Encode!(&key).unwrap();
        let res = self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "get_tx_from_map",
            args,
        )?;

        let tx = Decode!(&res.bytes(), Option<Transaction>)?;

        Ok(tx)
    }

    pub fn insert_tx_to_map(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        let res = self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "insert_tx_to_map",
            args,
        )?;

        let key = Decode!(&res.bytes(), u64)?;

        Ok(key)
    }

    pub fn get_tx_from_multimap(&self, key: u64) -> Result<Option<Transaction>> {
        let args = Encode!(&key).unwrap();
        let res = self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "get_tx_from_multimap",
            args,
        )?;

        let tx = Decode!(&res.bytes(), Option<Transaction>)?;

        Ok(tx)
    }

    pub fn insert_tx_to_multimap(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        let res = self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "insert_tx_to_multimap",
            args,
        )?;

        let key = Decode!(&res.bytes(), u64)?;

        Ok(key)
    }

    pub fn get_tx_from_vec(&self, index: u64) -> Result<Option<Transaction>> {
        let args = Encode!(&index).unwrap();
        let res = self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "get_tx_from_vec",
            args,
        )?;

        let tx = Decode!(&res.bytes(), Option<Transaction>)?;

        Ok(tx)
    }

    pub fn push_tx_to_vec(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        let res = self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "push_tx_to_vec",
            args,
        )?;

        let key = Decode!(&res.bytes(), u64)?;

        Ok(key)
    }

    pub fn get_tx_from_ring_buffer(&self, index: u64) -> Result<Option<Transaction>> {
        let args = Encode!(&index).unwrap();
        let res = self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "get_tx_from_ring_buffer",
            args,
        )?;

        let tx = Decode!(&res.bytes(), Option<Transaction>)?;

        Ok(tx)
    }

    pub fn push_tx_to_ring_buffer(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        let res = self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "push_tx_to_ring_buffer",
            args,
        )?;

        let key = Decode!(&res.bytes(), u64)?;

        Ok(key)
    }

    pub fn get_tx_from_log(&self, index: u64) -> Result<Option<Transaction>> {
        let args = Encode!(&index).unwrap();
        let res = self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "get_tx_from_log",
            args,
        )?;

        let tx = Decode!(&res.bytes(), Option<Transaction>)?;

        Ok(tx)
    }

    pub fn push_tx_to_log(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&Transaction { from, to, value }).unwrap();
        let res = self.env.execute_ingress_as(
            ic::caller().into(),
            self.dummy_canister,
            "push_tx_to_log",
            args,
        )?;

        let key = Decode!(&res.bytes(), u64)?;

        Ok(key)
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
        let env = StateMachine::new();
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

fn deploy_dummy_canister(env: &StateMachine) -> Result<CanisterId> {
    let dummy_wasm = get_dummy_canister_bytecode();
    eprintln!("Creating dummy canister");

    let args = Encode!(&())?;
    let canister = env.install_canister_with_cycles(
        dummy_wasm.to_vec(),
        args,
        None,
        Cycles::new(10_u128.pow(12)),
    )?;

    Ok(canister)
}

pub fn reinstall_dummy_canister(ctx: &StateMachineTestContext) -> Result<()> {
    let args = Encode!(&())?;

    let dummy_wasm = get_dummy_canister_bytecode();

    ctx.env
        .reinstall_canister(ctx.dummy_canister, dummy_wasm, args)?;

    Ok(())
}

pub fn upgrade_dummy_canister(ctx: &StateMachineTestContext) -> Result<()> {
    let args = Encode!(&())?;

    let dummy_wasm = get_dummy_canister_bytecode();

    ctx.env
        .upgrade_canister(ctx.dummy_canister, dummy_wasm, args)?;

    Ok(())
}
