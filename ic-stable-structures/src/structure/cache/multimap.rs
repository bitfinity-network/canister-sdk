use std::hash::Hash;

use dfinity_stable_structures::{Memory, Storable};

use crate::structure::*;

/// A LRU Cache for StableMultimaps
pub struct CachedStableMultimap<K1, K2, V, M>
where
    K1: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    K2: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord + Bounded,
    V: Storable + Clone + Send + Sync + 'static,
    M: Memory,
{
    inner: StableMultimap<K1, K2, V, M>,
    cache: SyncLruCache<(K1, K2), V>,
}

impl<K1, K2, V, M> CachedStableMultimap<K1, K2, V, M>
where
    K1: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    K2: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord + Bounded,
    V: Storable + Clone + Send + Sync + 'static,
    M: Memory,
{
    /// Create new instance of the CachedStableMultimap with a fixed number of max cached elements.
    pub fn new(memory: M, max_cache_items: u32) -> Self {
        Self::with_map(StableMultimap::new(memory), max_cache_items)
    }

    /// Create new instance of the CachedStableMultimap with a fixed number of max cached elements.
    pub fn with_map(inner: StableMultimap<K1, K2, V, M>, max_cache_items: u32) -> Self {
        Self {
            inner,
            cache: SyncLruCache::new(max_cache_items),
        }
    }

    /// Returns the inner collection so that the caller can have a readonly access to it that bypasses the cache.
    pub fn inner(&self) -> &StableMultimap<K1, K2, V, M> {
        &self.inner
    }
}

impl<K1, K2, V, M> MultimapStructure<K1, K2, V> for CachedStableMultimap<K1, K2, V, M>
where
    K1: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    K2: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord + Bounded,
    V: Storable + Clone + Send + Sync + 'static,
    M: Memory,
{
    type Iterator<'a>
        = <StableMultimap<K1, K2, V, M> as MultimapStructure<K1, K2, V>>::Iterator<'a>
    where
        Self: 'a;

    type RangeIterator<'a>
        = <StableMultimap<K1, K2, V, M> as MultimapStructure<K1, K2, V>>::RangeIterator<'a>
    where
        Self: 'a;

    fn get(&self, first_key: &K1, second_key: &K2) -> Option<V> {
        let key = (first_key.clone(), second_key.clone());

        self.cache
            .get_or_insert_with(&key, |_key| self.inner.get(first_key, second_key))
    }

    fn insert(&mut self, first_key: &K1, second_key: &K2, value: V) -> Option<V> {
        match self.inner.insert(first_key, second_key, value) {
            Some(old_value) => {
                let key = (first_key.clone(), second_key.clone());
                self.cache.remove(&key);
                Some(old_value)
            }
            None => None,
        }
    }

    fn remove(&mut self, first_key: &K1, second_key: &K2) -> Option<V> {
        match self.inner.remove(first_key, second_key) {
            Some(old_value) => {
                let key = (first_key.clone(), second_key.clone());
                self.cache.remove(&key);
                Some(old_value)
            }
            None => None,
        }
    }

    fn remove_partial(&mut self, first_key: &K1) -> bool {
        // Is it possible to remove only the partial keys?
        self.cache.clear();
        self.inner.remove_partial(first_key)
    }

    fn pop_first(&mut self) -> Option<((K1, K2), V)> {
        let res = self.inner.pop_first()?;
        self.cache.remove(&(res.0 .0.clone(), res.0 .1.clone()));
        Some(res)
    }

    fn pop_last(&mut self) -> Option<((K1, K2), V)> {
        let res = self.inner.pop_last()?;
        self.cache.remove(&(res.0 .0.clone(), res.0 .1.clone()));
        Some(res)
    }

    fn len(&self) -> u64 {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.cache.is_empty() && self.inner.is_empty()
    }

    fn clear(&mut self) {
        self.cache.clear();
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

        assert!(map.is_empty());

        assert_eq!(None, map.get(&1, &1));
        assert_eq!(None, map.get(&1, &2));
        assert_eq!(None, map.get(&2, &1));
        assert_eq!(None, map.get(&3, &1));

        assert_eq!(None, map.insert(&1, &1, Array([1u8, 1])));
        assert_eq!(None, map.insert(&1, &2, Array([1u8, 2])));
        assert_eq!(None, map.insert(&2, &1, Array([2u8, 1])));

        assert!(!map.is_empty());

        assert_eq!(Some(Array([1u8, 1])), map.get(&1, &1));
        assert_eq!(Some(Array([1u8, 1])), map.inner.get(&1, &1));
        assert_eq!(Some(Array([1u8, 2])), map.get(&1, &2));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2, &1));
        assert_eq!(None, map.get(&3, &1));

        assert_eq!(Some(Array([1u8, 1])), map.insert(&1, &1, Array([1u8, 10])));
        assert_eq!(Some(Array([2u8, 1])), map.insert(&2, &1, Array([2u8, 10])));

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

        assert_eq!(None, map.insert(&1, &1, Array([1u8, 1])));
        assert_eq!(None, map.insert(&2, &1, Array([2u8, 1])));
        assert_eq!(None, map.insert(&3, &1, Array([3u8, 1])));

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

        assert_eq!(None, map.insert(&1, &1, Array([1u8, 1])));
        assert_eq!(None, map.insert(&2, &1, Array([2u8, 1])));
        assert_eq!(None, map.insert(&3, &1, Array([3u8, 1])));
        assert_eq!(3, map.len());

        assert_eq!(Some(Array([1u8, 1])), map.get(&1, &1));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2, &1));

        assert_eq!(Some(Array([1u8, 1])), map.insert(&1, &1, Array([1u8, 10])));
        assert_eq!(Some(Array([3u8, 1])), map.insert(&3, &1, Array([3u8, 10])));

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

        map.insert(&1, &1, Array([1u8, 1]));
        map.insert(&1, &2, Array([2u8, 1]));
        map.insert(&3, &1, Array([3u8, 1]));

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

        map.insert(&1, &1, Array([1u8, 1]));
        map.insert(&1, &2, Array([2u8, 1]));
        map.insert(&3, &1, Array([3u8, 1]));

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

    #[test]
    fn test_pop_first_and_last_from_cache() {
        let cache_items = 10;

        let mut map = CachedStableMultimap::<u32, u32, Array<2>, _>::new(
            VectorMemory::default(),
            cache_items,
        );

        for i in 0..10 {
            map.insert(&i, &i, Array([i as u8, 1]));
        }

        assert_eq!(Some(((0, 0), Array([0u8, 1]))), map.pop_first());
        assert_eq!(Some(((9, 9), Array([9u8, 1]))), map.pop_last());

        assert_eq!(None, map.get(&0, &0));
        assert_eq!(None, map.get(&9, &9));
    }

    #[test]
    fn test_pop_first_and_last_not_cached() {
        let cache_items = 10;

        let mut map = CachedStableMultimap::<u32, u32, Array<2>, _>::new(
            VectorMemory::default(),
            cache_items,
        );

        for i in 0..cache_items * 3 {
            map.insert(&i, &i, Array([i as u8, 1]));
        }

        assert_eq!(Some(((0, 0), Array([0u8, 1]))), map.pop_first());
        assert_eq!(Some(((29, 29), Array([29u8, 1]))), map.pop_last());

        assert_eq!(None, map.get(&0, &0));
        assert_eq!(None, map.get(&29, &29));
    }

    #[test]
    fn should_get_and_insert_from_existing_amp() {
        let cache_items = 10;

        let mut map = CachedStableMultimap::<u32, u32, Array<2>, _>::new(
            VectorMemory::default(),
            cache_items,
        );

        map.inner.insert(&1, &1, Array([1u8, 1]));
        map.inner.insert(&2, &2, Array([2u8, 1]));

        assert!(!map.is_empty());

        assert_eq!(Some(Array([1u8, 1])), map.get(&1, &1));
        assert_eq!(Some(Array([1u8, 1])), map.remove(&1, &1));

        assert_eq!(None, map.get(&1, &1));
        assert_eq!(None, map.inner.get(&1, &1));

        assert!(!map.is_empty());

        assert_eq!(Some(Array([2u8, 1])), map.remove(&2, &2));

        assert!(map.get(&2, &2).is_none());
        assert!(map.inner.get(&2, &2).is_none());

        assert!(map.is_empty());

    }
}
