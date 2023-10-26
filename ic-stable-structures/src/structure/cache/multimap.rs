use std::hash::Hash;

use dfinity_stable_structures::{Memory, Storable};
use mini_moka::sync::{Cache, CacheBuilder};

use crate::structure::*;

/// A LRU Cache for StableMultimaps
pub struct CachedStableMultimap<K1, K2, V, M>
where
    K1: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    K2: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    V: Storable + Clone + Send + Sync + 'static,
    M: Memory,
{
    inner: StableMultimap<K1, K2, V, M>,
    cache: Cache<(K1, K2), V>,
}

impl<K1, K2, V, M> CachedStableMultimap<K1, K2, V, M>
where
    K1: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    K2: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    V: Storable + Clone + Send + Sync + 'static,
    M: Memory,
{
    /// Create new instance of the CachedStableMultimap with a fixed number of max cached elements.
    pub fn new(memory: M, max_cache_items: u64) -> Self {
        Self::with_map(StableMultimap::new(memory), max_cache_items)
    }

    /// Create new instance of the CachedStableMultimap with a fixed number of max cached elements.
    pub fn with_map(inner: StableMultimap<K1, K2, V, M>, max_cache_items: u64) -> Self {
        Self {
            inner,
            cache: CacheBuilder::default()
                    .max_capacity(max_cache_items)
                    .build(),
        }
    }
}

impl<K1, K2, V, M> MultimapStructure<K1, K2, V> for CachedStableMultimap<K1, K2, V, M>
where
    K1: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    K2: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    V: Storable + Clone + Send + Sync + 'static,
    M: Memory,
{
    type Iterator<'a> = <StableMultimap<K1, K2, V, M> as MultimapStructure<K1, K2, V>>::Iterator<'a> where Self: 'a;

    type RangeIterator<'a> = <StableMultimap<K1, K2, V, M> as MultimapStructure<K1, K2, V>>::RangeIterator<'a> where Self: 'a;

    fn get(&self, first_key: &K1, second_key: &K2) -> Option<V> {
        let key = (first_key.clone(), second_key.clone());

        match self.cache.get(&key) {
            Some(value) => Some(value.clone()),
            None => {
                let value = self.inner.get(first_key, second_key)?;
                self.cache.insert(key, value.clone());
                Some(value)
            }
        }
    }

    fn insert(&mut self, first_key: &K1, second_key: &K2, value: &V) -> Option<V> {
        match self.inner.insert(first_key, second_key, value) {
            Some(old_value) => {
                let key = (first_key.clone(), second_key.clone());
                self.cache.invalidate(&key);
                Some(old_value)
            }
            None => None,
        }
    }

    fn remove(&mut self, first_key: &K1, second_key: &K2) -> Option<V> {
        match self.inner.remove(first_key, second_key) {
            Some(old_value) => {
                let key = (first_key.clone(), second_key.clone());
                self.cache.invalidate(&key);
                Some(old_value)
            }
            None => None,
        }
    }

    fn remove_partial(&mut self, first_key: &K1) -> bool {

        let FIX_ME = 0;
        /// should remove only partial keys
        self.cache
            .invalidate_all();
        self.inner.remove_partial(first_key)
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn clear(&mut self) {
        self.cache.invalidate_all();
        self.inner.clear()
    }

    fn range(&self, first_key: &K1) -> Self::RangeIterator<'_> {
        self.inner.range(first_key)
    }

    fn iter(&self) -> Self::Iterator<'_> {
        self.inner.iter()
    }
}

#[cfg(test)]
mod test {

    use dfinity_stable_structures::VectorMemory;

    use super::*;
    use crate::test_utils::Array;

    #[test]
    fn should_get_and_insert() {
        let cache_items = 2;

        let mut map = CachedStableMultimap::<u32, u32, Array<2>, _>::new(
            VectorMemory::default(),
            cache_items,
        );

        assert_eq!(None, map.get(&1, &1));
        assert_eq!(None, map.get(&1, &2));
        assert_eq!(None, map.get(&2, &1));
        assert_eq!(None, map.get(&3, &1));

        assert_eq!(None, map.insert(&1, &1, &Array([1u8, 1])));
        assert_eq!(None, map.insert(&1, &2, &Array([1u8, 2])));
        assert_eq!(None, map.insert(&2, &1, &Array([2u8, 1])));

        assert_eq!(Some(Array([1u8, 1])), map.get(&1, &1));
        assert_eq!(Some(Array([1u8, 2])), map.get(&1, &2));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2, &1));
        assert_eq!(None, map.get(&3, &1));

        assert_eq!(Some(Array([1u8, 1])), map.insert(&1, &1, &Array([1u8, 10])));
        assert_eq!(Some(Array([2u8, 1])), map.insert(&2, &1, &Array([2u8, 10])));

        assert_eq!(Some(Array([1u8, 10])), map.get(&1, &1));
        assert_eq!(Some(Array([1u8, 2])), map.get(&1, &2));
        assert_eq!(Some(Array([2u8, 10])), map.get(&2, &1));
        assert_eq!(None, map.get(&3, &1));

        assert!(map.remove_partial(&1));
        assert!(!map.remove_partial(&1));

        assert_eq!(None, map.get(&1, &1));
        assert_eq!(None, map.get(&1, &2));
        assert_eq!(Some(Array([2u8, 10])), map.get(&2, &1));
        assert_eq!(None, map.get(&3, &1));
    }

    #[test]
    fn should_clear() {
        let cache_items = 2;

        let mut map = CachedStableMultimap::<u32, u32, Array<2>, _>::new(
            VectorMemory::default(),
            cache_items,
        );

        assert_eq!(None, map.insert(&1, &1, &Array([1u8, 1])));
        assert_eq!(None, map.insert(&2, &1, &Array([2u8, 1])));
        assert_eq!(None, map.insert(&3, &1, &Array([3u8, 1])));

        assert_eq!(Some(Array([1u8, 1])), map.get(&1, &1));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2, &1));

        map.clear();

        assert_eq!(0, map.len());

        assert_eq!(None, map.get(&1, &1));
        assert_eq!(None, map.get(&2, &1));
    }

    #[test]
    fn should_replace_old_value() {
        let cache_items = 2;

        let mut map = CachedStableMultimap::<u32, u32, Array<2>, _>::new(
            VectorMemory::default(),
            cache_items,
        );

        assert_eq!(None, map.insert(&1, &1, &Array([1u8, 1])));
        assert_eq!(None, map.insert(&2, &1, &Array([2u8, 1])));
        assert_eq!(None, map.insert(&3, &1, &Array([3u8, 1])));
        assert_eq!(3, map.len());

        assert_eq!(Some(Array([1u8, 1])), map.get(&1, &1));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2, &1));

        assert_eq!(Some(Array([1u8, 1])), map.insert(&1, &1, &Array([1u8, 10])));
        assert_eq!(Some(Array([3u8, 1])), map.insert(&3, &1, &Array([3u8, 10])));

        assert_eq!(Some(Array([1u8, 10])), map.get(&1, &1));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2, &1));
        assert_eq!(Some(Array([3u8, 10])), map.get(&3, &1));
    }

    #[test]
    fn iter() {
        let cache_items = 2;

        let mut map = CachedStableMultimap::<u32, u32, Array<2>, _>::new(
            VectorMemory::default(),
            cache_items,
        );

        map.insert(&1, &1, &Array([1u8, 1]));
        map.insert(&1, &2, &Array([2u8, 1]));
        map.insert(&3, &1, &Array([3u8, 1]));

        let mut iter = map.iter();
        assert_eq!(iter.next(), Some((1, 1, Array([1u8, 1]))));
        assert_eq!(iter.next(), Some((1, 2, Array([2u8, 1]))));
        assert_eq!(iter.next(), Some((3, 1, Array([3u8, 1]))));
    }

    #[test]
    fn range_iter() {
        let cache_items = 2;

        let mut map = CachedStableMultimap::<u32, u32, Array<2>, _>::new(
            VectorMemory::default(),
            cache_items,
        );

        map.insert(&1, &1, &Array([1u8, 1]));
        map.insert(&1, &2, &Array([2u8, 1]));
        map.insert(&3, &1, &Array([3u8, 1]));

        let mut iter = map.range(&1);
        assert_eq!(iter.next(), Some((1, Array([1u8, 1]))));
        assert_eq!(iter.next(), Some((2, Array([2u8, 1]))));
        assert_eq!(iter.next(), None);

        let mut iter = map.range(&2);
        assert_eq!(iter.next(), None);

        let mut iter = map.range(&3);
        assert_eq!(iter.next(), Some((1, Array([3u8, 1]))));
        assert_eq!(iter.next(), None);
    }
}
