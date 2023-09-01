use std::cell::RefCell;
use std::hash::Hash;

use ic_exports::stable_structures::{memory_manager::MemoryId, BoundedStorable};
use mini_moka::unsync::{Cache, CacheBuilder};

use crate::structure::*;

/// A LRU Cache for StableMultimaps
pub struct CachedStableMultimap<K1, K2, V>
where
    K1: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    K2: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    inner: StableMultimap<K1, K2, V>,
    cache: RefCell<Cache<(K1, K2), V>>,
}

impl<K1, K2, V> CachedStableMultimap<K1, K2, V>
where
    K1: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    K2: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    /// Create new instance of the CachedStableMultimap with a fixed number of max cached elements.
    pub fn new(memory_id: MemoryId, max_cache_items: u64) -> Self {
        Self::with_map(StableMultimap::new(memory_id), max_cache_items)
    }

    /// Create new instance of the CachedStableMultimap with a fixed number of max cached elements.
    pub fn with_map(inner: StableMultimap<K1, K2, V>, max_cache_items: u64) -> Self {
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

impl<K1, K2, V> MultimapStructure<K1, K2, V> for CachedStableMultimap<K1, K2, V>
where
    K1: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    K2: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    fn get(&self, first_key: &K1, second_key: &K2) -> Option<V> {
        let mut cache = self.cache.borrow_mut();
        let key = (first_key.clone(), second_key.clone());

        match cache.get(&key) {
            Some(value) => Some(value.clone()),
            None => match self.inner.get(first_key, second_key) {
                Some(value) => {
                    cache.insert(key, value.clone());
                    Some(value)
                }
                None => None,
            },
        }
    }

    fn insert(&mut self, first_key: &K1, second_key: &K2, value: &V) -> Option<V> {
        match self.inner.insert(first_key, second_key, value) {
            Some(old_value) => {
                let key = (first_key.clone(), second_key.clone());
                self.cache.borrow_mut().invalidate(&key);
                Some(old_value)
            }
            None => None,
        }
    }

    fn remove(&mut self, first_key: &K1, second_key: &K2) -> Option<V> {
        match self.inner.remove(first_key, second_key) {
            Some(old_value) => {
                let key = (first_key.clone(), second_key.clone());
                self.cache.borrow_mut().invalidate(&key);
                Some(old_value)
            }
            None => None,
        }
    }

    fn remove_partial(&mut self, first_key: &K1) -> bool {
        self.cache
            .borrow_mut()
            .invalidate_entries_if(|(k1, _k2), _v| k1 == first_key);
        self.inner.remove_partial(first_key)
    }

    fn len(&self) -> usize {
        self.inner.len() as usize
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
mod test {

    use crate::test_utils::Array;
    use ic_exports::stable_structures::memory_manager::MemoryId;

    use super::*;

    #[test]
    fn should_get_and_insert() {
        let cache_items = 2;
        let mut map =
            CachedStableMultimap::<u32, u32, Array<2>>::new(MemoryId::new(123), cache_items);

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
        let mut map: CachedStableMultimap<u32, u32, Array<2>> =
            CachedStableMultimap::<u32, u32, Array<2>>::new(MemoryId::new(101), cache_items);

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
        let mut map: CachedStableMultimap<u32, u32, Array<2>> =
            CachedStableMultimap::<u32, u32, Array<2>>::new(MemoryId::new(102), cache_items);

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
}
