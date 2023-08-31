use std::{collections::BTreeMap, hash::Hash};
use std::collections::btree_map::Iter as BTreeMapIter;
use ic_exports::stable_structures::{BoundedStorable, memory_manager::MemoryId};

use crate::SlicedStorable;

/// Stores key-value data in heap memory.
pub struct HeapUnboundedMap<K, V>(BTreeMap<K, V>)
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone;

impl<K, V> HeapUnboundedMap<K, V>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
{
    /// Create new instance of key-value storage.
    ///
    /// If a memory with the `memory_id` contains data of the map, the map reads it, and the instance
    /// will contain the data from the memory.
    pub fn new(_memory_id: MemoryId) -> Self {
        Self(BTreeMap::new())
    }

    /// Returns a value associated with `key` from heap memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn get(&self, key: &K) -> Option<V> {
        self.0.get(key).cloned()
    }

    /// Add or replace a value associated with `key` in stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `value.to_bytes().len() <= V::MAX_SIZE`
    pub fn insert(&mut self, key: &K, value: &V) -> Option<V> {
        self.0.insert(key.clone(), value.clone())
    }

    /// Remove a value associated with `key` from stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K1::MAX_SIZE`
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.0.remove(key)
    }

    /// List all currently stored key-value pairs.
    pub fn iter(&self) -> BTreeMapIter<'_, K, V> {
        self.0.iter()
    }

    /// Number of items in the map.
    pub fn len(&self) -> u64 {
        self.0.len() as u64
    }

    // Returns true if there are no values in the map.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Remove all entries from the map.
    pub fn clear(&mut self) {
        self.0.clear()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use ic_exports::stable_structures::memory_manager::MemoryId;

    use super::HeapUnboundedMap;
    use crate::test_utils;

    #[test]
    fn unbounded_map_works() {
        let mut map = HeapUnboundedMap::new(MemoryId::new(0));
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
        let expected = HashMap::from_iter([
            (&0u32, &long_str),
            (&3u32, &medium_str),
            (&5u32, &short_str),
        ]);
        assert_eq!(entries, expected);

        assert_eq!(map.remove(&3), Some(medium_str));

        assert_eq!(map.len(), 2);
    }

}
