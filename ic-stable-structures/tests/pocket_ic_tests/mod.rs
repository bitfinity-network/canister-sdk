use anyhow::Result;
use candid::{CandidType, Decode, Deserialize, Encode, Principal};
use did::*;
use ic_exports::ic_kit::mock_principals::alice;
use ic_exports::pocket_ic::{self, PocketIc, WasmResult};
use wasm_utils::get_dummy_canister_bytecode;

mod btreemap;
mod cached_btreemap;
mod cell;
mod log;
mod map;
mod multimap;
mod ring_buffer;
mod vec;
mod wasm_utils;

pub struct PocketIcTestContext {
    pub env: PocketIc,
    pub dummy_canister: Principal,
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

    pub fn get_tx_from_btreemap(&self, key: u64) -> Result<Option<BoundedTransaction>> {
        let args = Encode!(&key).unwrap();
        let res = self.query_as(alice(), self.dummy_canister, "get_tx_from_btreemap", args);

        Ok(res)
    }

    pub fn insert_tx_to_btreemap(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self.update_call_as(alice(), self.dummy_canister, "insert_tx_to_btreemap", args);

        Ok(res)
    }

    pub fn get_tx_from_cached_btreemap(&self, key: u64) -> Result<Option<BoundedTransaction>> {
        let args = Encode!(&key).unwrap();
        let res = self.query_as(
            alice(),
            self.dummy_canister,
            "get_tx_from_cached_btreemap",
            args,
        );

        Ok(res)
    }

    pub fn insert_tx_to_cached_btreemap(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self.update_call_as(
            alice(),
            self.dummy_canister,
            "insert_tx_to_cached_btreemap",
            args,
        );

        Ok(res)
    }

    pub fn get_tx_from_cell(&self) -> Result<BoundedTransaction> {
        let args = Encode!(&()).unwrap();
        let res = self.query_as(alice(), self.dummy_canister, "get_tx_from_cell", args);

        Ok(res)
    }

    pub fn insert_tx_to_cell(&self, from: u8, to: u8, value: u8) -> Result<BoundedTransaction> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self.update_call_as(alice(), self.dummy_canister, "insert_tx_to_cell", args);

        Ok(res)
    }

    pub fn get_tx_from_unboundedmap(&self, key: u64) -> Result<Option<UnboundedTransaction>> {
        let args = Encode!(&key).unwrap();
        let res = self.query_as(
            alice(),
            self.dummy_canister,
            "get_tx_from_unboundedmap",
            args,
        );

        Ok(res)
    }

    pub fn insert_tx_to_unboundedmap(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&UnboundedTransaction { from, to, value }).unwrap();
        let res = self.update_call_as(
            alice(),
            self.dummy_canister,
            "insert_tx_to_unboundedmap",
            args,
        );

        Ok(res)
    }

    pub fn get_tx_from_multimap(&self, key: u64) -> Result<Option<BoundedTransaction>> {
        let args = Encode!(&key).unwrap();
        let res = self.query_as(alice(), self.dummy_canister, "get_tx_from_multimap", args);

        Ok(res)
    }

    pub fn insert_tx_to_multimap(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self.update_call_as(alice(), self.dummy_canister, "insert_tx_to_multimap", args);

        Ok(res)
    }

    pub fn get_tx_from_vec(&self, index: u64) -> Result<Option<BoundedTransaction>> {
        let args = Encode!(&index).unwrap();
        let res = self.query_as(alice(), self.dummy_canister, "get_tx_from_vec", args);

        Ok(res)
    }

    pub fn push_tx_to_vec(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self.update_call_as(alice(), self.dummy_canister, "push_tx_to_vec", args);

        Ok(res)
    }

    pub fn get_tx_from_ring_buffer(&self, index: u64) -> Result<Option<BoundedTransaction>> {
        let args = Encode!(&index).unwrap();
        let res = self.query_as(
            alice(),
            self.dummy_canister,
            "get_tx_from_ring_buffer",
            args,
        );

        Ok(res)
    }

    pub fn push_tx_to_ring_buffer(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self.update_call_as(alice(), self.dummy_canister, "push_tx_to_ring_buffer", args);

        Ok(res)
    }

    pub fn get_tx_from_log(&self, index: u64) -> Result<Option<BoundedTransaction>> {
        let args = Encode!(&index).unwrap();
        let res = self.query_as(alice(), self.dummy_canister, "get_tx_from_log", args);

        Ok(res)
    }

    pub fn push_tx_to_log(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self.update_call_as(alice(), self.dummy_canister, "push_tx_to_log", args);

        Ok(res)
    }
}

pub fn with_pocket_ic_context<F>(f: F) -> Result<()>
where
    F: FnOnce(&PocketIcTestContext) -> Result<()>,
{
    let env = pocket_ic::init_pocket_ic();
    let dummy_canister = deploy_dummy_canister(&env).unwrap();

    let test_ctx = PocketIcTestContext {
        env,
        dummy_canister,
    };

    f(&test_ctx)?;

    Ok(())
}

fn deploy_dummy_canister(env: &PocketIc) -> Result<Principal> {
    let dummy_wasm = get_dummy_canister_bytecode();
    eprintln!("Creating dummy canister");

    let args = Encode!(&())?;

    let canister = env.create_canister();
    env.add_cycles(canister, 10_u128.pow(12));
    env.install_canister(canister, dummy_wasm.to_vec(), args, None);

    Ok(canister)
}

pub fn upgrade_dummy_canister(ctx: &PocketIcTestContext) -> Result<()> {
    let args = Encode!(&())?;

    let dummy_wasm = get_dummy_canister_bytecode();

    ctx.env
        .upgrade_canister(ctx.dummy_canister, dummy_wasm, args, None)
        .unwrap();

    Ok(())
}
