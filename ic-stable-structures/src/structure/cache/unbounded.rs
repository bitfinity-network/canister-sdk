use std::cell::RefCell;
use std::hash::Hash;

use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::BoundedStorable;

use crate::structure::*;
use mini_moka::unsync::{Cache, CacheBuilder};

/// A LRU Cache for StableUnboundedMaps
pub struct CachedStableUnboundedMap<K, V>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
{
    inner: StableUnboundedMap<K, V>,
    cache: RefCell<Cache<K, V>>,
}

impl<K, V> CachedStableUnboundedMap<K, V>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
{
    /// Create new instance of the CachedStableUnboundedMap with a fixed number of max cached elements.
    pub fn new(memory_id: MemoryId, max_cache_items: u64) -> Self {
        Self::with_map(StableUnboundedMap::new(memory_id), max_cache_items)
    }

    /// Create new instance of the CachedStableUnboundedMap with a fixed number of max cached elements.
    pub fn with_map(inner: StableUnboundedMap<K, V>, max_cache_items: u64) -> Self {
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

impl<K, V> UnboundedMapStructure<K, V> for CachedStableUnboundedMap<K, V>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
{
    fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.cache.borrow_mut();
        match cache.get(key) {
            Some(value) => Some(value.clone()),
            None => match self.inner.get(key) {
                Some(value) => {
                    cache.insert(key.clone(), value.clone());
                    Some(value)
                }
                None => None,
            },
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
    use ic_exports::stable_structures::memory_manager::MemoryId;

    use super::*;
    use crate::test_utils::{Array, StringValue};

    #[test]
    fn should_get_and_insert() {
        let cache_items = 2;
        let mut map =
            CachedStableUnboundedMap::<u32, StringValue>::new(MemoryId::new(123), cache_items);

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
        let mut map: CachedStableUnboundedMap<u32, Array<2>> =
            CachedStableUnboundedMap::<u32, Array<2>>::new(MemoryId::new(120), cache_items);

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
        let mut map: CachedStableUnboundedMap<u32, Array<2>> =
            CachedStableUnboundedMap::<u32, Array<2>>::new(MemoryId::new(121), cache_items);

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
        let mut map: CachedStableUnboundedMap<u32, Array<2>> =
            CachedStableUnboundedMap::<u32, Array<2>>::new(MemoryId::new(122), cache_items);

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
}
