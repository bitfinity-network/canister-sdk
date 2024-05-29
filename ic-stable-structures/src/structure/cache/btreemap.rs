use std::hash::Hash;

use dfinity_stable_structures::{Memory, Storable};

use crate::structure::*;

/// A LRU Cache for StableBTreeMap
pub struct CachedStableBTreeMap<K, V, M>
where
    K: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    V: Storable + Clone + Send + Sync + 'static,
    M: Memory,
{
    inner: StableBTreeMap<K, V, M>,
    cache: SyncLruCache<K, V>,
}

impl<K, V, M> CachedStableBTreeMap<K, V, M>
where
    K: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    V: Storable + Clone + Send + Sync + 'static,
    M: Memory,
{
    /// Create new instance of the CachedUnboundedMap with a fixed number of max cached elements.
    pub fn new(memory: M, max_cache_items: u32) -> Self {
        Self::with_map(StableBTreeMap::new(memory), max_cache_items)
    }

    /// Create new instance of the CachedUnboundedMap with a fixed number of max cached elements.
    pub fn with_map(inner: StableBTreeMap<K, V, M>, max_cache_items: u32) -> Self {
        Self {
            inner,
            cache: SyncLruCache::new(max_cache_items),
        }
    }

    /// Returns the inner collection so that the caller can have a readonly access to it that bypasses the cache.
    pub fn inner(&self) -> &StableBTreeMap<K, V, M> {
        &self.inner
    }
}

impl<K, V, M> BTreeMapStructure<K, V> for CachedStableBTreeMap<K, V, M>
where
    K: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    V: Storable + Clone + Send + Sync + 'static,
    M: Memory,
{
    fn get(&self, key: &K) -> Option<V> {
        self.cache
            .get_or_insert_with(key, |key| self.inner.get(key))
    }

    fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.inner.insert(key.clone(), value) {
            Some(old_value) => {
                self.cache.remove(&key);
                Some(old_value)
            }
            None => None,
        }
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        match self.inner.remove(key) {
            Some(old_value) => {
                self.cache.remove(key);
                Some(old_value)
            }
            None => None,
        }
    }

    fn len(&self) -> u64 {
        self.inner.len()
    }

    fn contains_key(&self, key: &K) -> bool {
        self.inner.contains_key(key)
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.inner.clear()
    }

    /// WARN: this bypasses the cache
    fn first_key_value(&self) -> Option<(K, V)> {
        self.inner.first_key_value()
    }

    /// WARN: this bypasses the cache
    fn last_key_value(&self) -> Option<(K, V)> {
        self.inner.last_key_value()
    }
}

/// NOTE: we can't implement this trait for a heap inner map because
/// `upper_bound` isn't implemented for `BTreeMap` in stable Rust
impl<K, V, M> IterableSortedMapStructure<K, V> for CachedStableBTreeMap<K, V, M>
where
    K: Storable + Clone + Send + Sync + Hash + Eq + PartialEq + Ord,
    V: Storable + Clone + Send + Sync,
    M: Memory,
{
    type Iterator<'a> = dfinity_stable_structures::btreemap::Iter<'a, K, V, M> where Self: 'a;

    fn iter(&self) -> Self::Iterator<'_> {
        self.inner.iter()
    }

    fn range(&self, key_range: impl RangeBounds<K>) -> Self::Iterator<'_> {
        self.inner.range(key_range)
    }

    fn iter_upper_bound(&self, bound: &K) -> Self::Iterator<'_> {
        self.inner.iter_upper_bound(bound)
    }
}

#[cfg(test)]
mod tests {
    use dfinity_stable_structures::VectorMemory;

    use super::*;
    use crate::test_utils::Array;

    #[test]
    fn should_get_and_insert() {
        let cache_items = 2;
        let mut map =
            CachedStableBTreeMap::<u32, Array<2>, _>::new(VectorMemory::default(), cache_items);

        assert_eq!(None, map.get(&1));
        assert_eq!(None, map.get(&2));
        assert_eq!(None, map.get(&3));
        assert_eq!(None, map.get(&4));

        assert_eq!(None, map.insert(1, Array([1u8, 1])));
        assert_eq!(None, map.insert(2, Array([2u8, 1])));
        assert_eq!(None, map.insert(3, Array([3u8, 1])));
        assert_eq!(3, map.len());

        assert_eq!(Some(Array([1u8, 1])), map.get(&1));

        assert_eq!(Some(Array([2u8, 1])), map.get(&2));

        assert_eq!(Some(Array([3u8, 1])), map.get(&3));

        assert_eq!(None, map.get(&4));

        assert_eq!(Some(Array([1u8, 1])), map.insert(1, Array([1u8, 10])));
        assert_eq!(Some(Array([2u8, 1])), map.insert(2, Array([2u8, 10])));
        assert_eq!(3, map.len());

        assert_eq!(Some(Array([2u8, 10])), map.get(&2));

        assert_eq!(Some(Array([1u8, 10])), map.get(&1));

        assert_eq!(Some(Array([3u8, 1])), map.get(&3));

        assert_eq!(None, map.get(&4));

        assert_eq!(Some(Array([1u8, 10])), map.remove(&1));
        assert_eq!(None, map.remove(&1));

        assert_eq!(None, map.get(&1));

        assert_eq!(Some(Array([2u8, 10])), map.remove(&2));
        assert_eq!(None, map.remove(&2));

        assert_eq!(None, map.get(&2));

        assert_eq!(None, map.get(&2));
        assert_eq!(Some(Array([3u8, 1])), map.get(&3));
        assert_eq!(None, map.get(&4));
    }

    #[test]
    fn should_clear() {
        let cache_items = 2;
        let mut map =
            CachedStableBTreeMap::<u32, Array<2>, _>::new(VectorMemory::default(), cache_items);

        assert_eq!(None, map.insert(1, Array([1u8, 1])));
        assert_eq!(None, map.insert(2, Array([2u8, 1])));
        assert_eq!(None, map.insert(3, Array([3u8, 1])));

        assert_eq!(Some(Array([1u8, 1])), map.get(&1));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2));

        map.clear();

        assert_eq!(0, map.len());

        assert_eq!(None, map.get(&1));
        assert_eq!(None, map.get(&2));
    }

    #[test]
    fn should_replace_old_value() {
        let cache_items = 2;
        let mut map =
            CachedStableBTreeMap::<u32, Array<2>, _>::new(VectorMemory::default(), cache_items);

        assert_eq!(None, map.insert(1, Array([1u8, 1])));
        assert_eq!(None, map.insert(2, Array([2u8, 1])));
        assert_eq!(None, map.insert(3, Array([3u8, 1])));
        assert_eq!(3, map.len());

        assert_eq!(Some(Array([1u8, 1])), map.get(&1));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2));

        assert_eq!(Some(Array([1u8, 1])), map.insert(1, Array([1u8, 10])));
        assert_eq!(Some(Array([3u8, 1])), map.insert(3, Array([3u8, 10])));

        assert_eq!(Some(Array([1u8, 10])), map.get(&1));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2));
        assert_eq!(Some(Array([3u8, 10])), map.get(&3));
    }

    #[test]
    fn should_iterate() {
        let cache_items = 2;
        let mut map =
            CachedStableBTreeMap::<u32, Array<2>, _>::new(VectorMemory::default(), cache_items);

        assert_eq!(None, map.insert(1, Array([1u8, 1])));
        assert_eq!(None, map.insert(2, Array([2u8, 1])));
        assert_eq!(None, map.insert(3, Array([3u8, 1])));

        let mut iter = map.iter();
        assert_eq!(iter.next(), Some((1, Array([1u8, 1]))));
        assert_eq!(iter.next(), Some((2, Array([2u8, 1]))));
        assert_eq!(iter.next(), Some((3, Array([3u8, 1]))));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn should_iterate_over_range() {
        let cache_items = 2;
        let mut map =
            CachedStableBTreeMap::<u32, Array<2>, _>::new(VectorMemory::default(), cache_items);

        assert_eq!(None, map.insert(1, Array([1u8, 1])));
        assert_eq!(None, map.insert(2, Array([2u8, 1])));
        assert_eq!(None, map.insert(3, Array([3u8, 1])));

        let mut iter = map.range(2..5);
        assert_eq!(iter.next(), Some((2, Array([2u8, 1]))));
        assert_eq!(iter.next(), Some((3, Array([3u8, 1]))));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn should_iterate_upper_bound() {
        let cache_items = 2;
        let mut map =
            CachedStableBTreeMap::<u32, Array<2>, _>::new(VectorMemory::default(), cache_items);

        assert_eq!(None, map.insert(1, Array([1u8, 1])));
        assert_eq!(None, map.insert(2, Array([2u8, 1])));
        assert_eq!(None, map.insert(3, Array([3u8, 1])));

        let mut iter = map.iter_upper_bound(&3);
        assert_eq!(iter.next(), Some((2, Array([2u8, 1]))));
        assert_eq!(iter.next(), Some((3, Array([3u8, 1]))));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_last_key_value() {
        let cache_items = 2;
        let mut map =
            CachedStableBTreeMap::<u32, u32, _>::new(VectorMemory::default(), cache_items);
        assert!(map.is_empty());

        assert!(map.last_key_value().is_none());

        map.insert(0u32, 42u32);
        assert_eq!(map.last_key_value(), Some((0, 42)));

        map.insert(10, 100);
        assert_eq!(map.last_key_value(), Some((10, 100)));

        map.insert(5, 100);
        assert_eq!(map.last_key_value(), Some((10, 100)));

        map.remove(&10);
        assert_eq!(map.last_key_value(), Some((5, 100)));
    }
}
