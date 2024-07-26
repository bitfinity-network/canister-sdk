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
            .env
            .query_call(canister_id, sender, method, payload)
            .await
            .unwrap()
        {
            WasmResult::Reply(bytes) => bytes,
            WasmResult::Reject(e) => panic!("Unexpected reject: {:?}", e),
        };

        Decode!(&res, Result).expect("failed to decode item from candid")
    }

    async fn update_call_as<Result>(
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
            .await
            .unwrap()
        {
            WasmResult::Reply(bytes) => bytes,
            WasmResult::Reject(e) => panic!("Unexpected reject: {:?}", e),
        };

        Decode!(&res, Result).expect("failed to decode item from candid")
    }

    pub async fn get_tx_from_btreemap(&self, key: u64) -> Result<Option<BoundedTransaction>> {
        let args = Encode!(&key).unwrap();
        let res = self
            .query_as(alice(), self.dummy_canister, "get_tx_from_btreemap", args)
            .await;

        Ok(res)
    }

    pub async fn insert_tx_to_btreemap(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self
            .update_call_as(alice(), self.dummy_canister, "insert_tx_to_btreemap", args)
            .await;

        Ok(res)
    }

    pub async fn get_tx_from_cached_btreemap(
        &self,
        key: u64,
    ) -> Result<Option<BoundedTransaction>> {
        let args = Encode!(&key).unwrap();
        let res = self
            .query_as(
                alice(),
                self.dummy_canister,
                "get_tx_from_cached_btreemap",
                args,
            )
            .await;

        Ok(res)
    }

    pub async fn insert_tx_to_cached_btreemap(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self
            .update_call_as(
                alice(),
                self.dummy_canister,
                "insert_tx_to_cached_btreemap",
                args,
            )
            .await;

        Ok(res)
    }

    pub async fn get_tx_from_cell(&self) -> Result<BoundedTransaction> {
        let args = Encode!(&()).unwrap();
        let res = self
            .query_as(alice(), self.dummy_canister, "get_tx_from_cell", args)
            .await;

        Ok(res)
    }

    pub async fn insert_tx_to_cell(
        &self,
        from: u8,
        to: u8,
        value: u8,
    ) -> Result<BoundedTransaction> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self
            .update_call_as(alice(), self.dummy_canister, "insert_tx_to_cell", args)
            .await;

        Ok(res)
    }

    pub async fn get_tx_from_unboundedmap(&self, key: u64) -> Result<Option<UnboundedTransaction>> {
        let args = Encode!(&key).unwrap();
        let res = self
            .query_as(
                alice(),
                self.dummy_canister,
                "get_tx_from_unboundedmap",
                args,
            )
            .await;

        Ok(res)
    }

    pub async fn insert_tx_to_unboundedmap(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&UnboundedTransaction { from, to, value }).unwrap();
        let res = self
            .update_call_as(
                alice(),
                self.dummy_canister,
                "insert_tx_to_unboundedmap",
                args,
            )
            .await;

        Ok(res)
    }

    pub async fn get_tx_from_multimap(&self, key: u64) -> Result<Option<BoundedTransaction>> {
        let args = Encode!(&key).unwrap();
        let res = self
            .query_as(alice(), self.dummy_canister, "get_tx_from_multimap", args)
            .await;

        Ok(res)
    }

    pub async fn insert_tx_to_multimap(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self
            .update_call_as(alice(), self.dummy_canister, "insert_tx_to_multimap", args)
            .await;

        Ok(res)
    }

    pub async fn get_tx_from_vec(&self, index: u64) -> Result<Option<BoundedTransaction>> {
        let args = Encode!(&index).unwrap();
        let res = self
            .query_as(alice(), self.dummy_canister, "get_tx_from_vec", args)
            .await;

        Ok(res)
    }

    pub async fn push_tx_to_vec(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self
            .update_call_as(alice(), self.dummy_canister, "push_tx_to_vec", args)
            .await;

        Ok(res)
    }

    pub async fn get_tx_from_ring_buffer(&self, index: u64) -> Result<Option<BoundedTransaction>> {
        let args = Encode!(&index).unwrap();
        let res = self
            .query_as(
                alice(),
                self.dummy_canister,
                "get_tx_from_ring_buffer",
                args,
            )
            .await;

        Ok(res)
    }

    pub async fn push_tx_to_ring_buffer(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self
            .update_call_as(alice(), self.dummy_canister, "push_tx_to_ring_buffer", args)
            .await;

        Ok(res)
    }

    pub async fn get_tx_from_log(&self, index: u64) -> Result<Option<BoundedTransaction>> {
        let args = Encode!(&index).unwrap();
        let res = self
            .query_as(alice(), self.dummy_canister, "get_tx_from_log", args)
            .await;

        Ok(res)
    }

    pub async fn push_tx_to_log(&self, from: u8, to: u8, value: u8) -> Result<u64> {
        let args = Encode!(&BoundedTransaction { from, to, value }).unwrap();
        let res = self
            .update_call_as(alice(), self.dummy_canister, "push_tx_to_log", args)
            .await;

        Ok(res)
    }
}

pub async fn new_test_context() -> PocketIcTestContext {
    let env = pocket_ic::init_pocket_ic().await;
    let dummy_canister = deploy_dummy_canister(&env).await.unwrap();

    PocketIcTestContext {
        env,
        dummy_canister,
    }
}

async fn deploy_dummy_canister(env: &PocketIc) -> Result<Principal> {
    let dummy_wasm = get_dummy_canister_bytecode();
    eprintln!("Creating dummy canister");

    let args = Encode!(&())?;

    let canister = env.create_canister().await;
    env.add_cycles(canister, 10_u128.pow(12)).await;
    env.install_canister(canister, dummy_wasm.to_vec(), args, None)
        .await;

    Ok(canister)
}

async fn upgrade_dummy_canister(ctx: &PocketIcTestContext) -> Result<()> {
    let args = Encode!(&())?;

    let dummy_wasm = get_dummy_canister_bytecode();

    ctx.env
        .upgrade_canister(ctx.dummy_canister, dummy_wasm, args, None)
        .await
        .unwrap();

    Ok(())
}
