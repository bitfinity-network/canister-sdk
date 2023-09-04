use std::collections::BTreeMap;

use ic_exports::stable_structures::{memory_manager::MemoryId, BoundedStorable};

use crate::structure::BTreeMapStructure;

/// Stores key-value data in heap memory.
pub struct HeapBTreeMap<K, V>(BTreeMap<K, V>)
where
    K: BoundedStorable + Ord + Clone,
    V: BoundedStorable + Clone;

impl<K, V> HeapBTreeMap<K, V>
where
    K: BoundedStorable + Ord + Clone,
    V: BoundedStorable + Clone,
{
    /// Create new instance of key-value storage.
    pub fn new(_memory_id: MemoryId) -> Self {
        Self(BTreeMap::new())
    }

    /// Iterate over all currently stored key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (K, V)> + '_ {
        self.0.iter().map(|(k, v)| (k.clone(), v.clone()))
    }
}

impl<K, V> BTreeMapStructure<K, V> for HeapBTreeMap<K, V>
where
    K: BoundedStorable + Ord + Clone,
    V: BoundedStorable + Clone,
{
    fn get(&self, key: &K) -> Option<V> {
        self.0.get(key).cloned()
    }

    fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        self.0.remove(key)
    }

    fn len(&self) -> u64 {
        self.0.len() as u64
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn clear(&mut self) {
        self.0.clear();
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use ic_exports::stable_structures::memory_manager::MemoryId;

    #[test]
    fn btreemap_works() {
        let mut map = HeapBTreeMap::new(MemoryId::new(0));
        assert!(map.is_empty());

        map.insert(0u32, 42u32);
        map.insert(10, 100);
        assert_eq!(map.get(&0), Some(42));
        assert_eq!(map.get(&10), Some(100));

        {
            let mut iter = map.iter();
            assert_eq!(iter.next(), Some((0, 42)));
            assert_eq!(iter.next(), Some((10, 100)));
            assert_eq!(iter.next(), None);
        }

        assert_eq!(map.remove(&10), Some(100));

        assert_eq!(map.len(), 1);
    }
}
