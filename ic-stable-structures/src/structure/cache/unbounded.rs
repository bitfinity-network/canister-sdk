use std::hash::Hash;

use dfinity_stable_structures::{Memory, Storable};

use crate::structure::stable_storage::StableUnboundedMap;
use crate::{SlicedStorable, StableUnboundedIter, SyncLruCache, UnboundedMapStructure};

/// A LRU Cache for StableUnboundedMaps
pub struct CachedStableUnboundedMap<K, V, M>
where
    K: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone + Send + Sync + 'static,
    M: Memory,
{
    inner: StableUnboundedMap<K, V, M>,
    cache: SyncLruCache<K, V>,
}

impl<K, V, M> CachedStableUnboundedMap<K, V, M>
where
    K: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone + Send + Sync + 'static,
    M: Memory,
{
    /// Create new instance of the CachedStableUnboundedMap with a fixed number of max cached elements.
    pub fn new(memory: M, max_cache_items: u32) -> Self {
        Self::with_map(StableUnboundedMap::new(memory), max_cache_items)
    }

    /// Create new instance of the CachedStableUnboundedMap with a fixed number of max cached elements.
    pub fn with_map(inner: StableUnboundedMap<K, V, M>, max_cache_items: u32) -> Self {
        Self {
            inner,
            cache: SyncLruCache::new(max_cache_items),
        }
    }

    /// Returns the inner collection so that the caller can have a readonly access to it that bypasses the cache.
    pub fn inner(&self) -> &StableUnboundedMap<K, V, M> {
        &self.inner
    }
}

impl<K, V, M> UnboundedMapStructure<K, V> for CachedStableUnboundedMap<K, V, M>
where
    K: Storable + Clone + Send + Sync + 'static + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone + Send + Sync + 'static,
    M: Memory,
{
    type Iterator<'a> = StableUnboundedIter<'a, K, V, M> where Self: 'a;

    fn get(&self, key: &K) -> Option<V> {
        self.cache
            .get_or_insert_with(key, |key| self.inner.get(key))
    }

    fn insert(&mut self, key: &K, value: &V) -> Option<V> {
        match self.inner.insert(key, value) {
            Some(old_value) => {
                self.cache.remove(key);
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

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.inner.clear()
    }

    /// WARN: this bypasses the cache
    fn first_key(&self) -> Option<K> {
        self.inner.first_key()
    }

    /// WARN: this bypasses the cache
    fn first_key_value(&self) -> Option<(K, V)> {
        self.inner.first_key_value()
    }

    /// WARN: this bypasses the cache
    fn last_key(&self) -> Option<K> {
        self.inner.last_key()
    }

    /// WARN: this bypasses the cache
    fn last_key_value(&self) -> Option<(K, V)> {
        self.inner.last_key_value()
    }

    fn iter(&self) -> Self::Iterator<'_> {
        self.inner.iter()
    }
}

#[cfg(test)]
mod tests {

    use dfinity_stable_structures::VectorMemory;

    use super::*;
    use crate::test_utils::{str_val, Array, StringValue};

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
    fn test_first_and_last_key_value() {
        let mut map = StableUnboundedMap::new(VectorMemory::default());
        assert!(map.is_empty());

        assert!(map.first_key().is_none());
        assert!(map.first_key_value().is_none());
        assert!(map.last_key().is_none());
        assert!(map.last_key_value().is_none());

        let str_0 = str_val(50000);
        map.insert(&0u32, &str_0);

        assert_eq!(map.first_key(), Some(0u32));
        assert_eq!(map.first_key_value(), Some((0u32, str_0.clone())));
        assert_eq!(map.last_key(), Some(0u32));
        assert_eq!(map.last_key_value(), Some((0u32, str_0.clone())));

        let str_3 = str_val(5000);
        map.insert(&3u32, &str_3);

        assert_eq!(map.first_key(), Some(0u32));
        assert_eq!(map.first_key_value(), Some((0u32, str_0.clone())));
        assert_eq!(map.last_key(), Some(3u32));
        assert_eq!(map.last_key_value(), Some((3u32, str_3.clone())));

        let str_5 = str_val(50);
        map.insert(&5u32, &str_5);

        assert_eq!(map.first_key(), Some(0u32));
        assert_eq!(map.first_key_value(), Some((0u32, str_0.clone())));
        assert_eq!(map.last_key(), Some(5u32));
        assert_eq!(map.last_key_value(), Some((5u32, str_5.clone())));

        let str_4 = str_val(50);
        map.insert(&4u32, &str_4);

        assert_eq!(map.first_key(), Some(0u32));
        assert_eq!(map.first_key_value(), Some((0u32, str_0.clone())));
        assert_eq!(map.last_key(), Some(5u32));
        assert_eq!(map.last_key_value(), Some((5u32, str_5)));

        map.remove(&5u32);

        assert_eq!(map.first_key(), Some(0u32));
        assert_eq!(map.first_key_value(), Some((0u32, str_0.clone())));
        assert_eq!(map.last_key(), Some(4u32));
        assert_eq!(map.last_key_value(), Some((4u32, str_4.clone())));

        let str_4_b = str_val(50);
        map.insert(&4u32, &str_4_b);

        assert_eq!(map.first_key(), Some(0u32));
        assert_eq!(map.first_key_value(), Some((0u32, str_0.clone())));
        assert_eq!(map.last_key(), Some(4u32));
        assert_eq!(map.last_key_value(), Some((4u32, str_4_b)));

        map.remove(&0u32);

        assert_eq!(map.first_key(), Some(3u32));
        assert_eq!(map.first_key_value(), Some((3u32, str_3)));
        assert_eq!(map.last_key(), Some(4u32));
        assert_eq!(map.last_key_value(), Some((4u32, str_4)));
    }
}
