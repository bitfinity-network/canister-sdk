use std::hash::Hash;

use dfinity_stable_structures::{Memory, Storable};
use mini_moka::unsync::{Cache, CacheBuilder};
use parking_lot::Mutex;

use crate::structure::*;

/// A LRU Cache for StableUnboundedMaps
pub struct CachedStableUnboundedMap<K, V, M>
where
    K: Storable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
    M: Memory,
{
    inner: StableUnboundedMap<K, V, M>,
    cache: Mutex<Cache<K, V>>,
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
            cache: Mutex::new(
                CacheBuilder::default()
                    .max_capacity(max_cache_items)
                    .build(),
            ),
        }
    }
}

impl<K, V, M> UnboundedMapStructure<K, V> for CachedStableUnboundedMap<K, V, M>
where
    K: Storable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
    M: Memory,
{
    fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.cache.lock();
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
                self.cache.lock().invalidate(key);
                Some(old_value)
            }
            None => None,
        }
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        match self.inner.remove(key) {
            Some(old_value) => {
                self.cache.lock().invalidate(key);
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
        self.cache.lock().invalidate_all();
        self.inner.clear()
    }
}

#[cfg(test)]
mod tests {

    use dfinity_stable_structures::VectorMemory;

    use super::*;
    use crate::test_utils::{Array, StringValue};

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
}
