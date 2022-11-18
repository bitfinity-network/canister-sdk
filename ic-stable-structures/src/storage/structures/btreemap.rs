use std::collections::HashMap;

use ic_exports::{
    ic_kit::ic,
    stable_structures::{btreemap, memory_manager::MemoryId, Storable},
    Principal,
};

use crate::{Memory, Result};

/// Stores key-value data in stable memory.
pub struct StableBTreeMap<K: Storable, V: Storable> {
    data: HashMap<Principal, btreemap::BTreeMap<Memory, K, V>>,
    memory_id: MemoryId,
    max_key_size: u32,
    max_value_size: u32,
    empty: btreemap::BTreeMap<Memory, K, V>,
}

impl<K: Storable, V: Storable> StableBTreeMap<K, V> {
    /// Create new instance of key-value storage.
    pub fn new(memory_id: MemoryId, max_key_size: u32, max_value_size: u32) -> Self {
        let memory = crate::get_memory_by_id(memory_id);
        let empty = btreemap::BTreeMap::init(memory, max_key_size, max_value_size);

        Self {
            data: HashMap::default(),
            memory_id,
            max_key_size,
            max_value_size,
            empty,
        }
    }

    /// Return value associated with `key` from stable memory.
    pub fn get(&self, key: &K) -> Option<V> {
        self.get_inner().get(key)
    }

    /// Add or replace value associated with `key` in stable memory.
    pub fn insert(&mut self, key: K, value: V) -> Result<()> {
        let canister_id = ic::id();

        // If map for `canister_id` is not initialized, initialize it.
        self.data
            .entry(canister_id)
            .or_insert_with(|| {
                let memory = crate::get_memory_by_id(self.memory_id);
                btreemap::BTreeMap::init(memory, self.max_key_size, self.max_value_size)
            })
            .insert(key, value)?;
        Ok(())
    }

    /// Remove value associated with `key` from stable memory.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.get_inner_mut().remove(key)
    }

    /// List all currently stored key-value pairs.
    pub fn iter(&self) -> btreemap::Iter<'_, Memory, K, V> {
        self.get_inner().iter()
    }

    fn get_inner(&self) -> &btreemap::BTreeMap<Memory, K, V> {
        let canister_id = ic::id();
        self.data.get(&canister_id).unwrap_or(&self.empty)
    }

    fn get_inner_mut(&mut self) -> &mut btreemap::BTreeMap<Memory, K, V> {
        let canister_id = ic::id();
        self.data.get_mut(&canister_id).unwrap_or(&mut self.empty)
    }
}
