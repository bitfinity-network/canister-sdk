use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    hash::Hash,
};

use crate::structure::*;
use ic_exports::stable_structures::{memory_manager::MemoryId, BoundedStorable};

/// A LRU Cache for StableBTreeMap
pub struct CachedStableBTreeMap<K, V>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    inner: StableBTreeMap<K, V>,
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

impl<K, V> CachedStableBTreeMap<K, V>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    /// Create new instance of the CachedUnboundedMap with a fixed number of max cached elements.
    pub fn new(memory_id: MemoryId, max_cache_items: usize) -> Self {
        Self::with_map(StableBTreeMap::new(memory_id), max_cache_items)
    }

    /// Create new instance of the CachedUnboundedMap with a fixed number of max cached elements.
    pub fn with_map(inner: StableBTreeMap<K, V>, max_cache_items: usize) -> Self {
        Self {
            inner,
            cache: RefCell::new(Cache::new(max_cache_items)),
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

impl<K, V> BTreeMapStructure<K, V> for CachedStableBTreeMap<K, V>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
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

    fn insert(&mut self, key: K, value: V) -> Option<V> {
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

    use super::*;
    use ic_exports::stable_structures::{memory_manager::MemoryId, BoundedStorable, Storable};
    use std::borrow::Cow;

    /// New type pattern used to implement `Storable` trait for all arrays.
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    struct Array<const N: usize>(pub [u8; N]);

    impl<const N: usize> Storable for Array<N> {
        fn to_bytes(&self) -> Cow<'_, [u8]> {
            Cow::Owned(self.0.to_vec())
        }

        fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
            let mut buf = [0u8; N];
            buf.copy_from_slice(&bytes);
            Array(buf)
        }
    }

    impl<const N: usize> BoundedStorable for Array<N> {
        const MAX_SIZE: u32 = N as _;
        const IS_FIXED_SIZE: bool = true;
    }

    #[test]
    fn should_get_and_insert() {
        let cache_items = 2;
        let mut map: CachedStableBTreeMap<u32, Array<2>> =
            CachedStableBTreeMap::<u32, Array<2>>::new(MemoryId::new(123), cache_items);

        assert_eq!(None, map.get(&1));
        assert_eq!(None, map.get(&2));
        assert_eq!(None, map.get(&3));
        assert_eq!(None, map.get(&4));

        assert_eq!(None, map.insert(1, Array([1u8, 1])));
        assert_eq!(None, map.insert(2, Array([2u8, 1])));
        assert_eq!(None, map.insert(3, Array([3u8, 1])));

        assert_eq!(Some(Array([1u8, 1])), map.get(&1));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2));
        assert_eq!(Some(Array([3u8, 1])), map.get(&3));
        assert_eq!(None, map.get(&4));

        assert_eq!(Some(Array([1u8, 1])), map.insert(1, Array([1u8, 10])));
        assert_eq!(Some(Array([2u8, 1])), map.insert(2, Array([2u8, 10])));

        assert_eq!(Some(Array([1u8, 10])), map.get(&1));
        assert_eq!(Some(Array([2u8, 10])), map.get(&2));
        assert_eq!(Some(Array([3u8, 1])), map.get(&3));
        assert_eq!(None, map.get(&4));

        assert_eq!(Some(Array([1u8, 10])), map.remove(&1));
        assert_eq!(None, map.remove(&1));

        assert_eq!(None, map.get(&1));
        assert_eq!(Some(Array([2u8, 10])), map.get(&2));
        assert_eq!(Some(Array([3u8, 1])), map.get(&3));
        assert_eq!(None, map.get(&4));
    }
}
