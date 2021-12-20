use crate::types::{Canister, Checksum};
use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;

/// Represents a state that manages canisters.
#[derive(CandidType, Clone, Serialize, Deserialize, Default)]
pub struct CanisterManager<K: Hash + Eq> {
    pub canisters: HashMap<K, Canister>,
    pub checksum: Checksum,
}

impl<K: Hash + Eq> CanisterManager<K> {
    /// Creates a new instance of `CanisterManager`.
    pub fn new(wasm_module: &[u8]) -> Self {
        Self {
            canisters: HashMap::new(),
            checksum: wasm_module.into(),
        }
    }

    /// This method should be called after restoring from stable memory.
    pub fn restore(&mut self, wasm_module: &[u8]) {
        self.checksum.upgrade(wasm_module.into());
    }

    /// Upgrades all canisters and returns a vector of outdated canisters.
    pub async fn upgrade(&mut self, wasm_module: &[u8]) -> Vec<Principal> {
        let mut outdated_canisters = Vec::new();

        for canister in self.canisters.values_mut() {
            if canister.version() == self.checksum.version {
                continue;
            }

            if canister
                .upgrade(self.checksum.version, wasm_module.into())
                .await
                .is_err()
            {
                outdated_canisters.push(canister.identity());
                continue;
            }
        }

        outdated_canisters
    }
}
