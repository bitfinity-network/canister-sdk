use std::collections::{VecDeque, HashMap};
use std::hash::Hash;

use ic_exports::ic_kit::ic;
use ic_exports::stable_structures::BoundedStorable;

use crate::{SlicedStorable, StableUnboundedMap, UnboundedIter, Memory};

/// Map that allows to store values with arbitrary size in stable memory.
///
/// Current implementation stores values in chunks with fixed size.
/// Size of chunk should be set using the [`SlicedStorable`] trait.
pub struct CachedUnboundedMap<K, V>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
{
    inner: StableUnboundedMap<K, V>,
    cache: HashMap<K, V>,
    cache_keys: VecDeque<K>,
    cache_items: usize,
}

impl<K, V> CachedUnboundedMap<K, V>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
{
    /// Create new instance of the map.
    ///
    /// If the `memory` contains data of the map, the map reads it, and the instance
    /// will contain the data from the `memory`.
    pub fn new( inner: StableUnboundedMap<K, V>, cache_items: usize) -> Self {
        Self {
            inner,
            cache: Default::default(),
            cache_keys: Default::default(),
            cache_items,
        }
    }

    /// Return a value associated with `key`.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn get(&self, key: &K) -> Option<V> {
        match self.cache.get(key) {
            Some(value) => {
                ic::print("CACHE HIT!!");
                Some(value.clone())
            },
            None => {
                ic::print("CACHE MISS!!");
                self.inner.get(key)
            },
        }
    }

    /// Add or replace a value associated with `key`.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn insert(&mut self, key: &K, value: &V) -> Option<V> {
        self.remove_oldest_from_cache();
        self.cache_keys.push_back(key.clone());
        self.cache.insert(key.clone(), value.clone());
        self.inner.insert(key, value)
    }

    #[inline]
    fn remove_oldest_from_cache(&mut self) {
        if self.cache.len() >= self.cache_items {
            match self.cache_keys.pop_front() {
                Some(key) => {
                    self.cache.remove(&key);
                },
                None => (),
            };
        }
    }

    /// Remove a value associated with `key`.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.cache.remove(key);
        self.inner.remove(key)
    }

    /// Iterator for all stored key-value pairs.
    pub fn iter(&self) -> UnboundedIter<'_, Memory, K, V> {
        self.inner.iter()
    }

    /// Count of items in the map.
    pub fn len(&self) -> u64 {
        self.inner.len()
    }

    /// Is the map empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Remove all entries from the map.
    pub fn clear(&mut self) {
        self.cache.clear();
        self.inner.clear()
    }
}



#[cfg(test)]
mod tests {
    use ic_exports::stable_structures::DefaultMemoryImpl;

    fn ADD_CACHE_TESTS() {}

//     use super::CachedUnboundedMap;
//     use crate::test_utils;

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
