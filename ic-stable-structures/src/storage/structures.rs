use std::collections::{hash_map::Entry, HashMap};

use ic_exports::candid::Principal;
use ic_exports::ic_kit::ic;
use ic_exports::stable_structures::{btreemap, cell, memory_manager::MemoryId, Storable};

use crate::multimap::{self, Iter, RangeIter};
use crate::{Result, Memory};

/// Stores value in stable memory, providing `get()/set()` API.
pub struct StableCell<T: Storable> {
    data: HashMap<Principal, cell::Cell<T, Memory>>,
    default_value: T,
    memory_id: MemoryId,
}

impl<T: Storable> StableCell<T> {
    /// Create new storage for values with `T` type.
    pub fn new(memory_id: MemoryId, value: T) -> Result<Self> {
        // Method returns Result to be compatible with wasm implementation.
        Ok(Self {
            data: HashMap::default(),
            default_value: value,
            memory_id,
        })
    }

    /// Returns reference to value stored in stable memory.
    pub fn get(&self) -> &T {
        let canister_id = ic::id();
        self.data
            .get(&canister_id)
            .map(|cell| cell.get())
            .unwrap_or(&self.default_value)
    }

    /// Updates value in stable memory.
    pub fn set(&mut self, value: T) -> Result<()> {
        let canister_id = ic::id();
        match self.data.entry(canister_id) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().set(value)?;
            }
            Entry::Vacant(entry) => {
                let memory = super::get_memory_by_id(self.memory_id);
                entry.insert(cell::Cell::init(memory, value)?);
            }
        };
        Ok(())
    }
}
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
                let memory = super::get_memory_by_id(self.memory_id);
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

/// [`StableMultimap`] stores two keys against a single value, making it possible
/// to fetch all values by the root key, or a single value by specifying both keys.
pub struct StableMultimap<K1, K2, V>
where
    K1: Storable,
    K2: Storable,
    V: Storable,
{
    maps: HashMap<Principal, multimap::StableMultimap<Memory, K1, K2, V>>,
    memory_id: MemoryId,
    max_first_key_size: u32,
    max_second_key_size: u32,
    max_value_size: u32,
    empty: multimap::StableMultimap<Memory, K1, K2, V>,
}

impl<K1, K2, V> StableMultimap<K1, K2, V>
where
    K1: Storable,
    K2: Storable,
    V: Storable,
{
    /// Create a new instance of a `StableMultimap`.
    /// All keys and values byte representations should be less then related `..._max_size` arguments.
    pub fn new(
        memory_id: MemoryId,
        max_first_key_size: u32,
        max_second_key_size: u32,
        max_value_size: u32,
    ) -> Self {
        let memory = crate::get_memory_by_id(memory_id);
        let empty = multimap::StableMultimap::new(
            memory,
            max_first_key_size,
            max_second_key_size,
            max_value_size,
        );

        Self {
            maps: HashMap::default(),
            memory_id,
            max_first_key_size,
            max_second_key_size,
            max_value_size,
            empty,
        }
    }

    /// Get a value for the given keys.
    /// If byte representation length of any key exceeds max size, `None` will be returned.
    pub fn get(&self, first_key: &K1, second_key: &K2) -> Option<V> {
        self.get_inner().get(first_key, second_key)
    }

    /// Insert a new value into the map.
    /// Inserting a value with the same keys as an existing value
    /// will result in the old value being overwritten.
    /// 
    /// # Errors
    /// 
    /// If byte representation length of any key or value exceeds max size, the `Error::ValueTooLarge`
    /// will be returned.
    /// 
    /// If stable memory unable to grow, the `Error::OutOfStableMemory` will be returned.
    pub fn insert(&mut self, first_key: &K1, second_key: &K2, value: &V) -> Result<()> {
        let canister_id = ic::id();

        // If map for `canister_id` is not initialized, initialize it.
        let map = self.maps.entry(canister_id).or_insert_with(|| {
            let memory = crate::get_memory_by_id(self.memory_id);
            multimap::StableMultimap::new(
                memory,
                self.max_first_key_size,
                self.max_second_key_size,
                self.max_value_size,
            )
        });

        map.insert(first_key, second_key, value)
    }

    /// Remove a specific value and return it.
    /// 
    /// # Errors
    /// 
    /// If byte representation length of any key exceeds max size, the `Error::ValueTooLarge`
    /// will be returned.
    pub fn remove(&mut self, first_key: &K1, second_key: &K2) -> Result<Option<V>> {
        self.get_inner_mut().remove(first_key, second_key)
    }

    /// Remove all values for the partial key
    /// 
    /// # Errors
    /// 
    /// If byte representation length of `first_key` exceeds max size, the `Error::ValueTooLarge`
    /// will be returned.
    pub fn remove_partial(&mut self, first_key: &K1) -> Result<()> {
        self.get_inner_mut().remove_partial(first_key)
    }

    /// Get a range of key value pairs based on the root key.
    /// 
    /// # Errors
    /// 
    /// If byte representation length of `first_key` exceeds max size, the `Error::ValueTooLarge`
    /// will be returned.
    pub fn range(&self, first_key: &K1) -> Result<RangeIter<Memory, K2, V>> {
        self.get_inner().range(first_key)
    }

    /// Iterator over all items in map.
    pub fn iter(&self) -> Iter<Memory, K1, K2, V> {
        self.get_inner().iter()
    }

    /// Items count.
    pub fn len(&self) -> usize {
        self.get_inner().len()
    }

    /// Is map empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn get_inner(&self) -> &multimap::StableMultimap<Memory, K1, K2, V> {
        let canister_id = ic::id();
        self.maps.get(&canister_id).unwrap_or(&self.empty)
    }

    fn get_inner_mut(&mut self) -> &mut multimap::StableMultimap<Memory, K1, K2, V> {
        let canister_id = ic::id();
        self.maps.get_mut(&canister_id).unwrap_or(&mut self.empty)
    }
}
