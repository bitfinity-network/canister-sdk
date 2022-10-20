use ic_exports::stable_structures::{btreemap, cell, memory_manager::MemoryId, Storable};

use crate::get_memory_by_id;
use crate::{error::Error, Memory};

/// Stores value in stable memory, providing `get()/set()` API.
pub struct StableCell<T: Storable>(cell::Cell<T, Memory>);

impl<T: Storable> StableCell<T> {
    /// Create new storage for values with `T` type.
    pub fn new(memory_id: MemoryId, value: T) -> Result<Self, Error> {
        let memory = super::get_memory_by_id(memory_id);
        let cell = cell::Cell::init(memory, value)?;
        Ok(Self(cell))
    }

    /// Returns reference to value stored in stable memory.
    pub fn get(&self) -> &T {
        self.0.get()
    }

    /// Updates value in stable memory.
    pub fn set(&mut self, value: T) -> Result<(), Error> {
        self.0.set(value)?;
        Ok(())
    }
}
/// Stores key-value data in stable memory.
pub struct StableBTreeMap<K: Storable, V: Storable>(btreemap::BTreeMap<Memory, K, V>);

impl<K: Storable, V: Storable> StableBTreeMap<K, V> {
    /// Create new instance of key-value storage.
    pub fn new(memory_id: MemoryId, max_key_size: u32, max_value_size: u32) -> Self {
        let memory = get_memory_by_id(memory_id);
        Self(btreemap::BTreeMap::init(
            memory,
            max_key_size,
            max_value_size,
        ))
    }

    /// Return value associated with `key` from stable memory.
    pub fn get(&self, key: &K) -> Option<V> {
        self.0.get(key)
    }

    /// Add or replace value associated with `key` in stable memory.
    pub fn insert(&mut self, key: K, value: V) -> Result<(), Error> {
        self.0.insert(key, value)?;
        Ok(())
    }

    /// Remove value associated with `key` from stable memory.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.0.remove(key)
    }

    /// List all currently stored key-value pairs.
    pub fn list(&self, start: usize, limit: usize) -> Vec<(K, V)> {
        self.0.iter().skip(start).take(limit).collect()
    }
}
