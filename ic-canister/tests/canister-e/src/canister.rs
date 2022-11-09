//! This example project shows how to export api from a canister and generate the idl with the API included.
//! We have the feature `export-api` which enables the canister APIs to be exported.
//! In the function  `generate_idl!` , we have a check to see if the feature `export-api` is enabled. If it is, we generate the idl with the API included, otherwise the APIs are not included.
//!
//! This means that we must have the feature  `export-api` enabled in `Cargo.toml` of the canister project.
//!
//! ```toml
//! [features]
//! default = []
//! export-api = []
//! ```
//!
//! When we are building the canister project, we must run/build with the feature `export-api` enabled. This is done by passing the flag `--features export-api` to the `cargo` command.
//! ```bash
//! cargo build --release --target wasm32-unknown-unknown --features export-api
//! ```
//! And we want to generate the idl with the API included, we must pass the flag `--features export-api` to the `cargo` command.
//! ```bash
//! cargo run --features export-api > canister_e.did
//! ```
//!
//!

use std::cell::RefCell;
use std::rc::Rc;

use candid::CandidType;
use candid::Deserialize;
use candid::Principal;
use ic_canister::generate_idl;
use ic_canister::query;
use ic_canister::update;
use ic_canister::Canister;
use ic_canister::Idl;
use ic_canister::PreUpdate;
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;

#[derive(Default, CandidType, Deserialize, IcStorage)]
pub struct State {
    counter: u32,
}

impl Versioned for State {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

#[derive(Canister)]
pub struct CounterCanister {
    #[id]
    id: Principal,
    #[state]
    counter: Rc<RefCell<State>>,
}

impl PreUpdate for CounterCanister {}

impl CounterCanister {
    #[query]
    pub fn get_counter(&self) -> u32 {
        self.counter.borrow().counter
    }

    #[update]
    pub fn inc_counter(&mut self, value: u32) {
        RefCell::borrow_mut(&self.counter).counter += value;
    }

    #[update]
    pub fn de_counter(&self) -> u32 {
        self.counter.borrow().counter - 1
    }

    /// Important: This function must be added to the canister to provide the idl.
    pub fn idl() -> Idl {
        generate_idl!()
    }
}
