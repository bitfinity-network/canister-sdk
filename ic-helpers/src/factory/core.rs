use crate::factory::error::FactoryError;
use crate::factory::types::{Canister, Checksum, Version};
use candid::utils::ArgumentEncoder;
use candid::{CandidType, Principal};
use ic_cdk::api::call::CallResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;

/// Represents a state that manages ic-helpers.
#[derive(CandidType, Debug, Clone, Serialize, Deserialize, Default)]
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

    /// Returns the number of ic-helpers cretaed by the factory.
    pub fn len(&self) -> usize {
        self.canisters.len()
    }

    /// Returns true if no canister is created yet.
    pub fn is_empty(&self) -> bool {
        self.canisters.is_empty()
    }

    /// Returns a vector of all ic-helpers created by the factory.
    pub fn all(&self) -> Vec<Principal> {
        self.canisters
            .values()
            .map(|canister| canister.identity())
            .collect()
    }

    // This method does ont work in IC main net because of how the cycles are consumed.
    #[deprecated(since = "0.2.0", note = "use create_with_cycles instead")]
    pub fn create<A: ArgumentEncoder>(
        &self,
        wasm_module: &[u8],
        arg: A,
    ) -> impl Future<Output = CallResult<Canister>> {
        Canister::create(self.checksum.version, wasm_module.into(), arg, 0)
    }

    /// Creates a pair with cycles in it to make it workable.
    ///
    /// The amount of cycles that will be available in the created canister is `cycles - FEE`, where
    /// `FEE` is a constant value needed to cover the factory expenses. Current implementation has
    /// `FEE == 10^11 + 10^6`.
    pub fn create_with_cycles<A: ArgumentEncoder>(
        &self,
        wasm_module: &[u8],
        arg: A,
        cycles: u64,
    ) -> impl Future<Output = CallResult<Canister>> {
        Canister::create(self.checksum.version, wasm_module.into(), arg, cycles)
    }

    /// Stops and deletes the canister. After this actor is awaited on, [Factory::forget] method must be used
    /// to remove the canister from the list of created canisters.
    pub fn drop(&self, canister: Principal) -> impl Future<Output = Result<(), FactoryError>> {
        drop_canister(canister)
    }

    /// Adds a new canister to the canister registry. If a canister with the given key is already
    /// registered, it will be replaced with the new one.
    pub fn register(&mut self, key: K, canister: Canister) {
        self.canisters.insert(key, canister);
    }

    /// Removes the canister from the registry. Return error if the canister with the given key is
    /// not registered.
    pub fn forget(&mut self, key: &K) -> Result<(), FactoryError> {
        self.canisters
            .remove(key)
            .ok_or(FactoryError::NotFound)
            .map(|_| ())
    }

    /// Returns a future that upgrades a canister to the given bytecode. After the future is
    /// done executing, `register_upgraded` method shall be called to add the resulting canister to the
    /// registry.
    ///
    /// Please, note that the state should not be borrowed when this future is awaited on, to prevent
    /// memory access conflict in case of concurrent requests.
    pub fn upgrade(
        &self,
        canister: &Canister,
        wasm_module: Vec<u8>,
    ) -> impl Future<Output = CallResult<Canister>> {
        upgrade_canister(self.checksum.version, canister.clone(), wasm_module)
    }

    /// Updates the canister to the newer version. If no canister with the given key is registered,
    /// nothing is done.
    pub fn register_upgraded(&mut self, key: &K, canister: Canister) {
        if let Some(val) = self.canisters.get_mut(key) {
            *val = canister;
        }
    }
}

async fn upgrade_canister(
    version: Version,
    mut canister: Canister,
    wasm_module: Vec<u8>,
) -> CallResult<Canister> {
    canister.upgrade(version, wasm_module).await?;
    Ok(canister)
}

async fn drop_canister(canister: Principal) -> Result<(), FactoryError> {
    let canister = crate::management::Canister::from(canister);
    canister
        .stop()
        .await
        .map_err(|(_, e)| FactoryError::ManagementError(e))?;
    canister
        .delete()
        .await
        .map_err(|(_, e)| FactoryError::ManagementError(e))?;

    Ok(())
}
