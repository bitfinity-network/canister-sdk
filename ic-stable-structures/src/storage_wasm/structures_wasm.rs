use ic_exports::stable_structures::{btreemap, cell, memory_manager::MemoryId, Storable};

use crate::{error::Error, Memory};
use crate::{get_memory_by_id, multimap, Iter, RangeIter};

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

pub struct StableMultimap<K1, K2, V>(multimap::StableMultimap<Memory, K1, K2, V>);

impl<K1, K2, V> StableMultimap<K1, K2, V>
where
    K1: Storable,
    K2: Storable,
    V: Storable,
{
    pub fn new(
        memory_id: MemoryId,
        max_first_key_size: u32,
        max_second_key_size: u32,
        max_value_size: u32,
    ) -> Self {
        let memory = crate::get_memory_by_id(memory_id);
        Self(multimap::StableMultimap::new(
            memory,
            max_first_key_size,
            max_second_key_size,
            max_value_size,
        ))
    }

    /// Return value associated with `key` from stable memory.
    pub fn get(&self, first_key: &K1, second_key: &K2) -> Option<V> {
        self.0.get(first_key, second_key)
    }

    /// Add or replace value associated with `key` in stable memory.
    pub fn insert(&mut self, first_key: &K1, second_key: &K2, value: V) -> Result<(), Error> {
        self.0.insert(first_key, second_key, value)
    }

    /// Remove value associated with `key` from stable memory.
    pub fn remove(&mut self, first_key: &K1, second_key: &K2) -> Result<Option<V>, Error> {
        self.0.remove(first_key, second_key)
    }

    /// Remove all values for the partial key
    pub fn remove_partial(&mut self, first_key: &K1) -> Result<(), Error> {
        self.0.remove_partial(first_key)
    }

    /// Get a range of key value pairs based on the root key.
    pub fn range(&self, first_key: &K1) -> Result<RangeIter<Memory, K2, V>, Error> {
        self.0.range(first_key)
    }

    /// Iterator over all items in map.
    pub fn iter(&self) -> Iter<Memory, K1, K2, V> {
        self.0.iter()
    }

    /// Items count.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Is map empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
