use std::collections::HashMap;

use ic_exports::{
    ic_kit::ic,
    stable_structures::{btreemap, memory_manager::MemoryId, BoundedStorable, Storable},
    Principal,
};

use crate::{Memory, Result};

/// Stores key-value data in stable memory.
pub struct StableBTreeMap<K: Storable, V: Storable> {
    data: HashMap<Principal, btreemap::BTreeMap<Memory, K, V>>,
    memory_id: MemoryId,
    empty: btreemap::BTreeMap<Memory, K, V>,
}

impl<K: BoundedStorable, V: BoundedStorable> StableBTreeMap<K, V> {
    /// Create new instance of key-value storage.
    pub fn new(memory_id: MemoryId) -> Self {
        let memory = crate::get_memory_by_id(memory_id);
        let empty = btreemap::BTreeMap::init(memory);

        Self {
            data: HashMap::default(),
            memory_id,
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
                btreemap::BTreeMap::init(memory)
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

    /// Count of items in the map.
    pub fn len(&self) -> u64 {
        self.get_inner().len()
    }

    /// Is the map empty.
    pub fn is_empty(&self) -> bool {
        self.get_inner().is_empty()
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

#[cfg(test)]
mod tests {
    use ic_exports::{
        ic_kit::{inject::get_context, mock_principals, MockContext},
        stable_structures::memory_manager::MemoryId,
    };

    use super::StableBTreeMap;

    #[test]
    fn map_works() {
        MockContext::new().inject();
        let mut map = StableBTreeMap::new(MemoryId::new(0));
        assert!(map.is_empty());

        map.insert(0u32, 42u32).unwrap();
        map.insert(10, 100).unwrap();
        assert_eq!(map.get(&0), Some(42));
        assert_eq!(map.get(&10), Some(100));

        let mut iter = map.iter();
        assert_eq!(iter.next(), Some((0, 42)));
        assert_eq!(iter.next(), Some((10, 100)));
        assert_eq!(iter.next(), None);

        assert_eq!(map.remove(&10), Some(100));

        assert_eq!(map.len(), 1);
    }

    #[test]
    fn two_canisters() {
        MockContext::new()
            .with_id(mock_principals::alice())
            .inject();

        let mut map = StableBTreeMap::new(MemoryId::new(0));
        map.insert(0u32, 42u32).unwrap();

        get_context().update_id(mock_principals::bob());
        map.insert(10, 100).unwrap();

        get_context().update_id(mock_principals::alice());
        assert_eq!(map.get(&0), Some(42));
        assert_eq!(map.len(), 1);

        get_context().update_id(mock_principals::bob());
        assert_eq!(map.get(&10), Some(100));
        assert_eq!(map.len(), 1);
    }
}
