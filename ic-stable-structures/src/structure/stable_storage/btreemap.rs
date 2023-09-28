use std::ops::RangeBounds;

use ic_exports::stable_structures::{btreemap, BoundedStorable, Memory};

use crate::structure::BTreeMapStructure;
use crate::IterableSortedMapStructure;

/// Stores key-value data in stable memory.
pub struct StableBTreeMap<K, V, M: Memory>(btreemap::BTreeMap<K, V, M>)
where
    K: BoundedStorable + Ord + Clone,
    V: BoundedStorable;

impl<K, V, M> StableBTreeMap<K, V, M>
where
    K: BoundedStorable + Ord + Clone,
    V: BoundedStorable,
    M: Memory
{
    /// Create new instance of key-value storage.
    pub fn new(memory: M) -> Self {
        Self(btreemap::BTreeMap::init(memory))
    }

    /// Iterate over all currently stored key-value pairs.
    pub fn iter(&self) -> btreemap::Iter<'_, K, V, M> {
        self.0.iter()
    }
}

impl<K, V, M> BTreeMapStructure<K, V> for StableBTreeMap<K, V, M>
where
    K: BoundedStorable + Ord + Clone,
    V: BoundedStorable,
    M: Memory,
{
    fn get(&self, key: &K) -> Option<V> {
        self.0.get(key)
    }

    fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        self.0.remove(key)
    }

    fn len(&self) -> u64 {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn clear(&mut self) {
        let inner = &mut self.0;

        let keys: Vec<_> = inner.iter().map(|(k, _)| k).collect();
        for key in keys {
            inner.remove(&key);
        }
    }
}

impl<K, V, M> IterableSortedMapStructure<K, V> for StableBTreeMap<K, V, M>
where
    K: BoundedStorable + Ord + Clone,
    V: BoundedStorable,
    M: Memory,
{
    type Iterator<'a> = btreemap::Iter<'a, K, V, M> where Self: 'a;

    fn iter(&self) -> Self::Iterator<'_> {
        self.0.iter()
    }

    fn range(&self, key_range: impl RangeBounds<K>) -> Self::Iterator<'_> {
        self.0.range(key_range)
    }

    fn iter_upper_bound(&self, bound: &K) -> Self::Iterator<'_> {
        self.0.iter_upper_bound(bound)
    }
}

#[cfg(test)]
mod tests {

    use ic_exports::stable_structures::VectorMemory;

    use super::*;

    #[test]
    fn btreemap_works() {
        let mut map = StableBTreeMap::new(VectorMemory::default());
        assert!(map.is_empty());

        map.insert(0u32, 42u32);
        map.insert(10, 100);
        assert_eq!(map.get(&0), Some(42));
        assert_eq!(map.get(&10), Some(100));

        let mut iter = map.iter();
        assert_eq!(iter.next(), Some((0, 42)));
        assert_eq!(iter.next(), Some((10, 100)));
        assert_eq!(iter.next(), None);

        let mut iter = map.range(1..11);
        assert_eq!(iter.next(), Some((10, 100)));
        assert_eq!(iter.next(), None);

        let mut iter = map.iter_upper_bound(&5);
        assert_eq!(iter.next(), Some((0, 42)));

        assert_eq!(map.remove(&10), Some(100));

        assert_eq!(map.len(), 1);
    }
}
