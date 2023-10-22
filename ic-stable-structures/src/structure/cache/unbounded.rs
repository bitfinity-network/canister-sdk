use std::cell::RefCell;
use std::hash::Hash;

use dfinity_stable_structures::{Memory, Storable};
use mini_moka::unsync::{Cache, CacheBuilder};

use crate::structure::stable_storage::{StableUnboundedIter, StableUnboundedMap};
use crate::SlicedStorable;
use crate::UnboundedMapStructure;

/// A LRU Cache for StableUnboundedMaps
pub struct CachedStableUnboundedMap<K, V, M>
where
    K: Storable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
    M: Memory,
{
    inner: StableUnboundedMap<K, V, M>,
    cache: RefCell<Cache<K, V>>,
}

impl<K, V, M> CachedStableUnboundedMap<K, V, M>
where
    K: Storable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
    M: Memory,
{
    /// Create new instance of the CachedStableUnboundedMap with a fixed number of max cached elements.
    pub fn new(memory: M, max_cache_items: u64) -> Self {
        Self::with_map(StableUnboundedMap::new(memory), max_cache_items)
    }

    /// Create new instance of the CachedStableUnboundedMap with a fixed number of max cached elements.
    pub fn with_map(inner: StableUnboundedMap<K, V, M>, max_cache_items: u64) -> Self {
        Self {
            inner,
            cache: RefCell::new(
                CacheBuilder::default()
                    .max_capacity(max_cache_items)
                    .build(),
            ),
        }
    }

    /// Iterator for all stored key-value pairs.
    pub fn iter(&self) -> StableUnboundedIter<'_, K, V, M> {
        self.inner.iter()
    }

    /// Returns an iterator pointing to the first element below the given bound.
    /// Returns an empty iterator if there are no keys below the given bound.
    pub fn iter_upper_bound(&self, bound: &K) -> StableUnboundedIter<'_, K, V, M> {
        self.inner.iter_upper_bound(bound)
    }
}

impl<K, V, M> UnboundedMapStructure<K, V> for CachedStableUnboundedMap<K, V, M>
where
    K: Storable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
    M: Memory,
{
    fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.cache.borrow_mut();
        match cache.get(key) {
            Some(value) => Some(value.clone()),
            None => {
                let value = self.inner.get(key)?;
                cache.insert(key.clone(), value.clone());
                Some(value)
            }
        }
    }

    fn insert(&mut self, key: &K, value: &V) -> Option<V> {
        match self.inner.insert(key, value) {
            Some(old_value) => {
                self.cache.borrow_mut().invalidate(key);
                Some(old_value)
            }
            None => None,
        }
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        match self.inner.remove(key) {
            Some(old_value) => {
                self.cache.borrow_mut().invalidate(key);
                Some(old_value)
            }
            None => None,
        }
    }

    fn len(&self) -> u64 {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn clear(&mut self) {
        self.cache.borrow_mut().invalidate_all();
        self.inner.clear()
    }
}

#[cfg(test)]
mod tests {

    use dfinity_stable_structures::VectorMemory;

    use super::*;
    use crate::test_utils::{self, Array, StringValue};

    #[test]
    fn should_get_and_insert() {
        let cache_items = 2;
        let mut map = CachedStableUnboundedMap::<u32, StringValue, _>::new(
            VectorMemory::default(),
            cache_items,
        );

        assert!(map.get(&1).is_none());
        assert!(map.get(&2).is_none());
        assert!(map.get(&3).is_none());
        assert!(map.get(&4).is_none());

        assert_eq!(None, map.insert(&1, &StringValue("one".to_string())));
        assert_eq!(None, map.insert(&2, &StringValue("two".to_string())));
        assert_eq!(None, map.insert(&3, &StringValue("three".to_string())));

        assert_eq!(Some(StringValue("one".to_string())), map.get(&1));
        assert_eq!(Some(StringValue("two".to_string())), map.get(&2));
        assert_eq!(Some(StringValue("three".to_string())), map.get(&3));
        assert!(map.get(&4).is_none());

        assert_eq!(
            Some(StringValue("one".to_string())),
            map.insert(&1, &StringValue("one_2".to_string()))
        );
        assert_eq!(
            Some(StringValue("two".to_string())),
            map.insert(&2, &StringValue("two_2".to_string()))
        );

        assert_eq!(Some(StringValue("one_2".to_string())), map.get(&1));
        assert_eq!(Some(StringValue("two_2".to_string())), map.get(&2));
        assert_eq!(Some(StringValue("three".to_string())), map.get(&3));
        assert!(map.get(&4).is_none());

        assert_eq!(Some(StringValue("one_2".to_string())), map.remove(&1));
        assert_eq!(None, map.remove(&1));

        assert_eq!(None, map.get(&1));
        assert_eq!(Some(StringValue("two_2".to_string())), map.get(&2));
        assert_eq!(Some(StringValue("three".to_string())), map.get(&3));
        assert!(map.get(&4).is_none());
    }

    #[test]
    fn should_get_insert_and_replace() {
        let cache_items = 2;
        let mut map =
            CachedStableUnboundedMap::<u32, Array<2>, _>::new(VectorMemory::default(), cache_items);

        assert_eq!(None, map.get(&1));
        assert_eq!(None, map.get(&2));
        assert_eq!(None, map.get(&3));
        assert_eq!(None, map.get(&4));

        assert_eq!(None, map.insert(&1, &Array([1u8, 1])));
        assert_eq!(None, map.insert(&2, &Array([2u8, 1])));
        assert_eq!(None, map.insert(&3, &Array([3u8, 1])));
        assert_eq!(3, map.len());

        assert_eq!(Some(Array([1u8, 1])), map.get(&1));

        assert_eq!(Some(Array([2u8, 1])), map.get(&2));

        assert_eq!(Some(Array([3u8, 1])), map.get(&3));

        assert_eq!(None, map.get(&4));

        assert_eq!(Some(Array([1u8, 1])), map.insert(&1, &Array([1u8, 10])));
        assert_eq!(Some(Array([2u8, 1])), map.insert(&2, &Array([2u8, 10])));
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
            CachedStableUnboundedMap::<u32, Array<2>, _>::new(VectorMemory::default(), cache_items);

        assert_eq!(None, map.insert(&1, &Array([1u8, 1])));
        assert_eq!(None, map.insert(&2, &Array([2u8, 1])));
        assert_eq!(None, map.insert(&3, &Array([3u8, 1])));

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
            CachedStableUnboundedMap::<u32, Array<2>, _>::new(VectorMemory::default(), cache_items);

        assert_eq!(None, map.insert(&1, &Array([1u8, 1])));
        assert_eq!(None, map.insert(&2, &Array([2u8, 1])));
        assert_eq!(None, map.insert(&3, &Array([3u8, 1])));
        assert_eq!(3, map.len());

        assert_eq!(Some(Array([1u8, 1])), map.get(&1));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2));

        assert_eq!(Some(Array([1u8, 1])), map.insert(&1, &Array([1u8, 10])));
        assert_eq!(Some(Array([3u8, 1])), map.insert(&3, &Array([3u8, 10])));

        assert_eq!(Some(Array([1u8, 10])), map.get(&1));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2));
        assert_eq!(Some(Array([3u8, 10])), map.get(&3));
    }

    #[test]
    fn iter_test() {
        let cache_items = 2;
        let mut map = CachedStableUnboundedMap::new(VectorMemory::default(), cache_items);

        let strs = [
            test_utils::str_val(50),
            test_utils::str_val(5000),
            test_utils::str_val(50000),
        ];

        for i in 0..100u32 {
            map.insert(&i, &strs[i as usize % strs.len()]);
        }

        assert!(map.iter().all(|(k, v)| v == strs[k as usize % strs.len()]))
    }

    #[test]
    fn upper_bound_test() {
        let cache_items = 2;
        let mut map = CachedStableUnboundedMap::new(VectorMemory::default(), cache_items);

        let strs = [
            test_utils::str_val(50),
            test_utils::str_val(5000),
            test_utils::str_val(50000),
        ];

        for i in 0..100u32 {
            map.insert(&i, &strs[i as usize % strs.len()]);
        }

        for i in 1..100u32 {
            let mut iter = map.iter_upper_bound(&i);
            assert_eq!(
                iter.next(),
                Some((i - 1, strs[(i - 1) as usize % strs.len()].clone()))
            );
        }

        let mut iter = map.iter_upper_bound(&0);
        assert_eq!(iter.next(), None);
    }
}
