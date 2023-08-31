
use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::{btreemap, BoundedStorable};

use crate::Memory;
use super::get_memory_by_id;

/// Stores key-value data in stable memory.
pub struct StableBTreeMap<K, V>(btreemap::BTreeMap<K, V, Memory>)
where
    K: BoundedStorable + Ord + Clone,
    V: BoundedStorable;

impl<K, V> StableBTreeMap<K, V>
where
    K: BoundedStorable + Ord + Clone,
    V: BoundedStorable,
{
    /// Create new instance of key-value storage.
    pub fn new(memory_id: MemoryId) -> Self {
        let memory = get_memory_by_id(memory_id);
        Self(btreemap::BTreeMap::init(memory))
    }

    /// Return value associated with `key` from stable memory.
    pub fn get(&self, key: &K) -> Option<V> {
        self.0.get(key)
    }

    /// Add or replace value associated with `key` in stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    ///   - `value.to_bytes().len() <= V::MAX_SIZE`
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }

    /// Remove value associated with `key` from stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.0.remove(key)
    }

    /// Iterate over all currently stored key-value pairs.
    pub fn iter(&self) -> btreemap::Iter<'_, K, V, Memory> {
        self.0.iter()
    }

    /// Count of items in the map.
    pub fn len(&self) -> u64 {
        self.0.len()
    }

    /// Is the map empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Remove all entries from the map.
    pub fn clear(&mut self) {
        let inner = &mut self.0;

        let keys: Vec<_> = inner.iter().map(|(k, _)| k).collect();
        for key in keys {
            inner.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    
    use ic_exports::stable_structures::memory_manager::MemoryId;
    use super::*;

    #[test]
    fn btreemap_works() {
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


}
