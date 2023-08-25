use std::collections::HashMap;

use ic_exports::candid::Principal;
use ic_exports::ic_kit::ic;
use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::BoundedStorable;

use crate::unbounded::{self, SlicedStorable};
use crate::Memory;

/// Stores key-value data in stable memory.
pub struct StableUnboundedMap<K, V>
where
    K: BoundedStorable,
    V: SlicedStorable,
{
    data: HashMap<Principal, unbounded::StableUnboundedMap<Memory, K, V>>,
    memory_id: MemoryId,
    empty: unbounded::StableUnboundedMap<Memory, K, V>,
}

impl<K, V> StableUnboundedMap<K, V>
where
    K: BoundedStorable,
    V: SlicedStorable,
{
    /// Create new instance of key-value storage.
    ///
    /// If a memory with the `memory_id` contains data of the map, the map reads it, and the instance
    /// will contain the data from the memory.
    pub fn new(memory_id: MemoryId) -> Self {
        let memory = crate::get_memory_by_id(memory_id);
        let empty = unbounded::StableUnboundedMap::new(memory);

        Self {
            data: HashMap::default(),
            memory_id,
            empty,
        }
    }

    /// Return value associated with `key` from stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn get(&self, key: &K) -> Option<V> {
        self.get_inner().get(key)
    }

    /// Add or replace value associated with `key` in stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn insert(&mut self, key: &K, value: &V) -> Option<V> {
        let canister_id = ic::id();

        // If map for `canister_id` is not initialized, initialize it.
        self.data
            .entry(canister_id)
            .or_insert_with(|| {
                let memory = crate::get_memory_by_id(self.memory_id);
                unbounded::StableUnboundedMap::new(memory)
            })
            .insert(key, value)
    }

    /// Remove value associated with `key` from stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.mut_inner().remove(key)
    }

    /// List all currently stored key-value pairs.
    pub fn iter(&self) -> unbounded::Iter<'_, Memory, K, V> {
        self.get_inner().iter()
    }

    /// Count of items in the map.
    pub fn len(&self) -> u64 {
        self.get_inner().len()
    }

    /// Is the map empty.
    pub fn is_empty(&self) -> bool {
        self.get_inner().is_empty()
    }

    /// Remove all entries from the map.
    pub fn clear(&mut self) {
        self.mut_inner().clear()
    }

    fn get_inner(&self) -> &unbounded::StableUnboundedMap<Memory, K, V> {
        let canister_id = ic::id();
        self.data.get(&canister_id).unwrap_or(&self.empty)
    }

    fn mut_inner(&mut self) -> &mut unbounded::StableUnboundedMap<Memory, K, V> {
        let canister_id = ic::id();
        self.data.get_mut(&canister_id).unwrap_or(&mut self.empty)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use ic_exports::ic_kit::inject::get_context;
    use ic_exports::ic_kit::{mock_principals, MockContext};
    use ic_exports::stable_structures::memory_manager::MemoryId;

    use super::StableUnboundedMap;
    use crate::test_utils;

    #[test]
    fn unbounded_map_works() {
        MockContext::new().inject();
        let mut map = StableUnboundedMap::new(MemoryId::new(0));
        assert!(map.is_empty());

        let long_str = test_utils::str_val(50000);
        let medium_str = test_utils::str_val(5000);
        let short_str = test_utils::str_val(50);

        map.insert(&0u32, &long_str);
        map.insert(&3u32, &medium_str);
        map.insert(&5u32, &short_str);
        assert_eq!(map.get(&0).as_ref(), Some(&long_str));
        assert_eq!(map.get(&3).as_ref(), Some(&medium_str));
        assert_eq!(map.get(&5).as_ref(), Some(&short_str));

        let entries: HashMap<_, _> = map.iter().collect();
        let expected = HashMap::from_iter([(0, long_str), (3, medium_str.clone()), (5, short_str)]);
        assert_eq!(entries, expected);

        assert_eq!(map.remove(&3), Some(medium_str));

        assert_eq!(map.len(), 2);
    }

    #[test]
    fn two_canisters() {
        MockContext::new()
            .with_id(mock_principals::alice())
            .inject();

        let mut map = StableUnboundedMap::new(MemoryId::new(0));

        let long_str = test_utils::str_val(50000);
        let medium_str = test_utils::str_val(5000);

        map.insert(&0u32, &long_str);

        get_context().update_id(mock_principals::bob());
        map.insert(&3u32, &medium_str);

        get_context().update_id(mock_principals::alice());
        assert_eq!(map.get(&0), Some(long_str));
        assert_eq!(map.len(), 1);

        get_context().update_id(mock_principals::bob());
        assert_eq!(map.get(&3), Some(medium_str));
        assert_eq!(map.len(), 1);
    }
}
