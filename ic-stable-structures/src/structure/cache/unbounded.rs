use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use ic_exports::stable_structures::BoundedStorable;

use crate::structure::stable_storage::SlicedStorable;
use crate::structure::UnboundedMapStructure;

/// A LRU Cache for UnboundedStructures
pub struct CachedUnboundedMap<K, V, MAP>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
    MAP: UnboundedMapStructure<K, V>,
{
    inner: MAP,
    cache: RefCell<Cache<K, V>>,
}

struct Cache<K, V> {
    cache: HashMap<K, V>,
    cache_keys: VecDeque<K>,
    cache_max_items: usize,
}

impl<K, V> Cache<K, V> {
    fn new(cache_max_items: usize) -> Self {
        Self {
            cache_max_items,
            cache: Default::default(),
            cache_keys: Default::default(),
        }
    }
}

impl<K, V, MAP> CachedUnboundedMap<K, V, MAP>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
    MAP: UnboundedMapStructure<K, V>,
{
    /// Create new instance of the map with a fixed number of max cached elements.
    ///
    pub fn new(inner: MAP, cache_items: usize) -> Self {
        Self {
            inner,
            cache: RefCell::new(Cache::new(cache_items)),
        }
    }

    #[inline]
    fn remove_oldest_from_cache(&self, cache: &mut Cache<K, V>) {
        if cache.cache_keys.len() > cache.cache_max_items {
            if let Some(key) = cache.cache_keys.pop_front() {
                cache.cache.remove(&key);
            };
        }
    }
}

impl<K, V, MAP> UnboundedMapStructure<K, V> for CachedUnboundedMap<K, V, MAP>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
    MAP: UnboundedMapStructure<K, V>,
{
    fn get(&self, key: &K) -> Option<V> {
        let cache = self.cache.borrow();
        match cache.cache.get(key) {
            Some(value) => Some(value.clone()),
            None => {
                drop(cache);
                match self.inner.get(key) {
                    Some(value) => {
                        {
                            let mut cache = self.cache.borrow_mut();
                            cache.cache.insert(key.clone(), value.clone());
                            cache.cache_keys.push_back(key.clone());
                            self.remove_oldest_from_cache(&mut cache);
                        }
                        Some(value)
                    }
                    None => None,
                }
            }
        }
    }

    fn insert(&mut self, key: &K, value: &V) -> Option<V> {
        self.inner.insert(key, value)
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        {
            let mut cache = self.cache.borrow_mut();
            if cache.cache.remove(key).is_some() {
                if let Some(pos) = cache.cache_keys.iter().position(|k| k == key) {
                    cache.cache_keys.remove(pos);
                }
            }
        }
        self.inner.remove(key)
    }

    fn len(&self) -> u64 {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn clear(&mut self) {
        {
            let mut cache = self.cache.borrow_mut();
            cache.cache.clear();
            cache.cache_keys.clear();
        }
        self.inner.clear()
    }
}

#[cfg(test)]
mod tests {
    use ic_exports::stable_structures::memory_manager::MemoryId;

    use super::*;
    use crate::{test_utils::StringValue, StableUnboundedMap};

    #[test]
    fn should_get_and_insert() {
        let cache_items = 2;
        let mut map = CachedUnboundedMap::<u32, StringValue, _>::new(
            StableUnboundedMap::new(MemoryId::new(123)),
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

    //     #[test]
    //     fn insert_get_test() {
    //         let mut map = CachedUnboundedMap::new(DefaultMemoryImpl::default());
    //         assert!(map.is_empty());

    //         let long_str = test_utils::str_val(50000);
    //         let medium_str = test_utils::str_val(5000);
    //         let short_str = test_utils::str_val(50);

    //         map.insert(&0u32, &long_str);
    //         map.insert(&3u32, &medium_str);
    //         map.insert(&5u32, &short_str);

    //         assert_eq!(map.get(&0).as_ref(), Some(&long_str));
    //         assert_eq!(map.get(&3).as_ref(), Some(&medium_str));
    //         assert_eq!(map.get(&5).as_ref(), Some(&short_str));
    //     }

    //     #[test]
    //     fn insert_should_replace_previous_value() {
    //         let mut map = CachedUnboundedMap::new(DefaultMemoryImpl::default());
    //         assert!(map.is_empty());

    //         let long_str = test_utils::str_val(50000);
    //         let short_str = test_utils::str_val(50);

    //         assert!(map.insert(&0u32, &long_str).is_none());
    //         let prev = map.insert(&0u32, &short_str).unwrap();

    //         assert_eq!(&prev, &long_str);
    //         assert_eq!(map.get(&0).as_ref(), Some(&short_str));
    //     }

    //     #[test]
    //     fn remove_test() {
    //         let mut map = CachedUnboundedMap::new(DefaultMemoryImpl::default());

    //         let long_str = test_utils::str_val(50000);
    //         let medium_str = test_utils::str_val(5000);
    //         let short_str = test_utils::str_val(50);

    //         map.insert(&0u32, &long_str);
    //         map.insert(&3u32, &medium_str);
    //         map.insert(&5u32, &short_str);

    //         assert_eq!(map.remove(&3), Some(medium_str));

    //         assert_eq!(map.get(&0).as_ref(), Some(&long_str));
    //         assert_eq!(map.get(&5).as_ref(), Some(&short_str));
    //         assert_eq!(map.len(), 2);
    //     }

    //     #[test]
    //     fn iter_test() {
    //         let mut map = CachedUnboundedMap::new(DefaultMemoryImpl::default());

    //         let strs = [
    //             test_utils::str_val(50),
    //             test_utils::str_val(5000),
    //             test_utils::str_val(50000),
    //         ];

    //         for i in 0..100u32 {
    //             map.insert(&i, &strs[i as usize % strs.len()]);
    //         }

    //         assert!(map.iter().all(|(k, v)| v == strs[k as usize % strs.len()]))
    //     }
}
