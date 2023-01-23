use std::collections::HashMap;

use ic_exports::ic_kit::ic;
use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::{btreemap, BoundedStorable};
use ic_exports::Principal;

use crate::Memory;

/// Stores key-value data in stable memory.
pub struct StableBTreeMap<K, V>
where
    K: BoundedStorable + Ord + Clone,
    V: BoundedStorable,
{
    data: HashMap<Principal, btreemap::BTreeMap<K, V, Memory>>,
    memory_id: MemoryId,
    empty: btreemap::BTreeMap<K, V, Memory>,
}

impl<K, V> StableBTreeMap<K, V>
where
    K: BoundedStorable + Ord + Clone,
    V: BoundedStorable,
{
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
    /// 
    /// # Preconditions:
    ///   - key.to_bytes().len() <= Key::MAX_SIZE
    ///   - value.to_bytes().len() <= Value::MAX_SIZE
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let canister_id = ic::id();

        // If map for `canister_id` is not initialized, initialize it.
        self.data
            .entry(canister_id)
            .or_insert_with(|| {
                let memory = crate::get_memory_by_id(self.memory_id);
                btreemap::BTreeMap::init(memory)
            })
            .insert(key, value)
    }

    /// Remove value associated with `key` from stable memory.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.mut_inner().remove(key)
    }

    /// List all currently stored key-value pairs.
    pub fn iter(&self) -> btreemap::Iter<'_, K, V, Memory> {
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
        let inner = self.mut_inner();

        let keys: Vec<_> = inner.iter().map(|(k, _)| k).collect();
        for key in keys {
            inner.remove(&key);
        }
    }

    fn get_inner(&self) -> &btreemap::BTreeMap<K, V, Memory> {
        let canister_id = ic::id();
        self.data.get(&canister_id).unwrap_or(&self.empty)
    }

    fn mut_inner(&mut self) -> &mut btreemap::BTreeMap<K, V, Memory> {
        let canister_id = ic::id();
        self.data.get_mut(&canister_id).unwrap_or(&mut self.empty)
    }
}

#[cfg(test)]
mod tests {
    use ic_exports::ic_kit::inject::get_context;
    use ic_exports::ic_kit::{mock_principals, MockContext};
    use ic_exports::stable_structures::memory_manager::MemoryId;

    use super::StableBTreeMap;

    #[test]
    fn map_works() {
        MockContext::new().inject();
        let mut map = StableBTreeMap::new(MemoryId::new(0));
        assert!(map.is_empty());

        map.insert(0u32, 42u32);
        map.insert(10, 100);
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
        map.insert(0u32, 42u32);

        get_context().update_id(mock_principals::bob());
        map.insert(10, 100);

        get_context().update_id(mock_principals::alice());
        assert_eq!(map.get(&0), Some(42));
        assert_eq!(map.len(), 1);

        get_context().update_id(mock_principals::bob());
        assert_eq!(map.get(&10), Some(100));
        assert_eq!(map.len(), 1);
    }
}
