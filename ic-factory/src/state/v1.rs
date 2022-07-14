use super::FactoryConfiguration;
use crate::types::{Canister, Checksum};
use ic_cdk::export::candid::{CandidType, Deserialize, Principal};
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;
use std::collections::HashMap;

#[derive(CandidType, Deserialize, IcStorage, Default)]
pub struct FactoryStateV1 {
    pub configuration: FactoryConfiguration,
    pub factory: Factory,
}

impl Versioned for FactoryStateV1 {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

#[derive(CandidType, Deserialize, Default)]
pub struct Factory {
    pub canisters: HashMap<Principal, Canister>,
    pub checksum: Checksum,
}
