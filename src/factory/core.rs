use crate::factory::types::{Canister, Checksum};
use candid::utils::ArgumentEncoder;
use candid::{CandidType, Principal};
use ic_cdk::api::call::CallResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;

/// Represents a state that manages canisters.
#[derive(CandidType, Clone, Serialize, Deserialize, Default)]
pub struct Factory<K: Hash + Eq> {
    pub canisters: HashMap<K, Canister>,
    pub checksum: Checksum,
}

impl<K: Hash + Eq> Factory<K> {
    /// Creates a new instance of `Factory`.
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

    /// Returns a canister that has been created by the factory.
    pub fn get(&self, key: &K) -> Option<Principal> {
        self.canisters.get(key).map(|canister| canister.identity())
    }

    /// Returns the number of canisters cretaed by the factory.
    pub fn len(&self) -> usize {
        self.canisters.len()
    }

    /// Returns true if no canister is created yet.
    pub fn is_empty(&self) -> bool {
        self.canisters.is_empty()
    }

    /// Returns a vector of all canisters created by the factory.
    pub fn all(&self) -> Vec<Principal> {
        self.canisters
            .values()
            .map(|canister| canister.identity())
            .collect()
    }

    /// Creates a new canister if it has not already created, and installs wasm_module on it.
    pub async fn create<A: ArgumentEncoder>(
        &mut self,
        key: K,
        wasm_module: &[u8],
        arg: A,
    ) -> CallResult<Principal> {
        if let Some(canister) = self.canisters.get(&key) {
            return Ok(canister.identity());
        }

        let canister = Canister::create(self.checksum.version, wasm_module.into(), arg).await?;

        let principal = canister.identity();
        self.canisters.insert(key, canister);
        Ok(principal)
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
