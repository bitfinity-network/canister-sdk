use std::{cell::RefCell, hash::Hash};

use crate::{structure::*, Memory};
use ic_exports::stable_structures::{memory_manager::MemoryId, BoundedStorable};
use mini_moka::unsync::{Cache, CacheBuilder};

/// A LRU Cache for StableBTreeMap
pub struct CachedStableBTreeMap<K, V>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    inner: StableBTreeMap<K, V>,
    cache: RefCell<Cache<K, V>>,
}

impl<K, V> CachedStableBTreeMap<K, V>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    /// Create new instance of the CachedUnboundedMap with a fixed number of max cached elements.
    pub fn new(memory_id: MemoryId, max_cache_items: u64) -> Self {
        Self::with_map(StableBTreeMap::new(memory_id), max_cache_items)
    }

    /// Create new instance of the CachedUnboundedMap with a fixed number of max cached elements.
    pub fn with_map(inner: StableBTreeMap<K, V>, max_cache_items: u64) -> Self {
        Self {
            inner,
            cache: RefCell::new(
                CacheBuilder::default()
                    .max_capacity(max_cache_items)
                    .build(),
            ),
        }
    }
}

impl<K, V> BTreeMapStructure<K, V> for CachedStableBTreeMap<K, V>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
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

    fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.inner.insert(key.clone(), value) {
            Some(old_value) => {
                self.cache.borrow_mut().invalidate(&key);
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

impl<K, V> IterableSortedMapStructure<K, V> for CachedStableBTreeMap<K, V>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    type Iterator<'a> = ic_exports::stable_structures::btreemap::Iter<'a, K, V, Memory> where Self: 'a;

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

    use crate::test_utils::Array;

    use super::*;
    use ic_exports::stable_structures::memory_manager::MemoryId;

    #[test]
    fn should_get_and_insert() {
        let cache_items = 2;
        let mut map: CachedStableBTreeMap<u32, Array<2>> =
            CachedStableBTreeMap::<u32, Array<2>>::new(MemoryId::new(100), cache_items);

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
        let mut map: CachedStableBTreeMap<u32, Array<2>> =
            CachedStableBTreeMap::<u32, Array<2>>::new(MemoryId::new(101), cache_items);

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
        let mut map: CachedStableBTreeMap<u32, Array<2>> =
            CachedStableBTreeMap::<u32, Array<2>>::new(MemoryId::new(102), cache_items);

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
        let mut map: CachedStableBTreeMap<u32, Array<2>> =
            CachedStableBTreeMap::<u32, Array<2>>::new(MemoryId::new(102), cache_items);

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
        let mut map: CachedStableBTreeMap<u32, Array<2>> =
            CachedStableBTreeMap::<u32, Array<2>>::new(MemoryId::new(102), cache_items);

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
        let mut map: CachedStableBTreeMap<u32, Array<2>> =
            CachedStableBTreeMap::<u32, Array<2>>::new(MemoryId::new(102), cache_items);

        assert_eq!(None, map.insert(1, Array([1u8, 1])));
        assert_eq!(None, map.insert(2, Array([2u8, 1])));
        assert_eq!(None, map.insert(3, Array([3u8, 1])));

        let mut iter = map.iter_upper_bound(&3);
        assert_eq!(iter.next(), Some((2, Array([2u8, 1]))));
        assert_eq!(iter.next(), Some((3, Array([3u8, 1]))));
        assert_eq!(iter.next(), None);
    }
}
