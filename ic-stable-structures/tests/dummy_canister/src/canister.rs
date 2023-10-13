use candid::Principal;
use did::Transaction;
use ic_canister::{generate_idl, init, query, update, Canister, Idl, PreUpdate};

use service::Service;

mod service;

#[derive(Canister)]
pub struct DummyCanister {
    #[id]
    id: Principal,
}

impl PreUpdate for DummyCanister {}

impl DummyCanister {
    #[init]
    pub fn init(&self) {
        Service::init()
    }

    #[query]
    pub fn get_tx_from_btreemap(&self, key: u64) -> Option<Transaction> {
        Service::get_tx_from_btreemap(key)
    }

    #[update]
    pub async fn insert_tx_to_btreemap(&self, transaction: Transaction) -> u64 {
        Service::insert_tx_to_btreemap(transaction)
    }

    #[query]
    pub fn get_tx_from_cell(&self) -> Transaction {
        Service::get_tx_from_cell()
    }

    #[update]
    pub async fn insert_tx_to_cell(&self, transaction: Transaction) -> Transaction {
        Service::insert_tx_to_cell(transaction);
        transaction
    }

    #[query]
    pub fn get_tx_from_log(&self, idx: u64) -> Option<Transaction> {
        Service::get_tx_from_log(idx)
    }

    #[update]
    pub async fn push_tx_to_log(&self, transaction: Transaction) -> u64 {
        Service::push_tx_to_log(transaction)
    }

    // #[query]
    // pub fn get_tx_from_map(&self, key: u64) -> Option<Transaction> {
    //     Service::get_tx_from_map(key)
    // }

    // #[update]
    // pub async fn insert_tx_to_map(&self, transaction: Transaction) -> u64 {
    //     Service::insert_tx_to_map(transaction)
    // }

    #[query]
    pub fn get_tx_from_multimap(&self, key: u64) -> Option<Transaction> {
        Service::get_tx_from_multimap(key)
    }

    #[update]
    pub async fn insert_tx_to_multimap(&self, transaction: Transaction) -> u64 {
        Service::insert_tx_to_multimap(transaction)
    }

    #[query]
    pub fn get_tx_from_vec(&self, idx: u64) -> Option<Transaction> {
        Service::get_tx_from_vec(idx)
    }

    #[update]
    pub async fn push_tx_to_vec(&self, transaction: Transaction) -> u64 {
        Service::push_tx_to_vec(transaction)
    }

    #[query]
    pub fn get_tx_from_ring_buffer(&self, idx: u64) -> Option<Transaction> {
        Service::get_tx_from_ring_buffer(idx)
    }

    #[update]
    pub async fn push_tx_to_ring_buffer(&self, transaction: Transaction) -> u64 {
        Service::push_tx_to_ring_buffer(transaction)
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}
