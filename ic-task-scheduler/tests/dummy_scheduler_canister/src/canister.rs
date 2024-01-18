use candid::Principal;
use ic_canister::{generate_idl, init, query, Canister, Idl, PreUpdate};

#[derive(Canister)]
pub struct DummyCanister {
    #[id]
    id: Principal,
}

impl PreUpdate for DummyCanister {}

impl DummyCanister {
    #[init]
    pub fn init(&self) {}

    #[query]
    pub fn save_state(&self) -> bool {
        true
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}
