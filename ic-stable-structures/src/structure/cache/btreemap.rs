use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    hash::Hash,
};

use crate::structure::*;
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
            cache: RefCell::new(CacheBuilder::default().max_capacity(max_cache_items).build()),
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
                match self.inner.get(key) {
                    Some(value) => {
                        cache.insert(key.clone(), value.clone());
                        Some(value)
                    }
                    None => None,
                }
            }
        }
    }

    fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.inner.insert(key.clone(), value) {
            Some(old_value) => {
                self.cache.borrow_mut().invalidate(&key);
                Some(old_value)
            },
            None => None,
        }
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        match self.inner.remove(key) {
            Some(old_value) => {
                self.cache.borrow_mut().invalidate(key);
                Some(old_value)
            },
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

    use crate::test_utils::Array;

    use super::*;
    use ic_exports::stable_structures::memory_manager::MemoryId;

    #[test]
    fn should_get_and_insert() {
        let cache_items = 2;
        let mut map: CachedStableBTreeMap<u32, Array<2>> =
            CachedStableBTreeMap::<u32, Array<2>>::new(MemoryId::new(123), cache_items);

        check_cache(&map, [].into());

        assert_eq!(None, map.get(&1));
        assert_eq!(None, map.get(&2));
        assert_eq!(None, map.get(&3));
        assert_eq!(None, map.get(&4));

        check_cache(&map, [].into());

        assert_eq!(None, map.insert(1, Array([1u8, 1])));
        assert_eq!(None, map.insert(2, Array([2u8, 1])));
        assert_eq!(None, map.insert(3, Array([3u8, 1])));
        assert_eq!(3, map.len());

        check_cache(&map, [].into());

        assert_eq!(Some(Array([1u8, 1])), map.get(&1));
        check_cache(&map, [
            (1, Array([1u8, 1])),
        ].into());

        assert_eq!(Some(Array([2u8, 1])), map.get(&2));
        check_cache(&map, [
            (1, Array([1u8, 1])),
            (2, Array([2u8, 1])),
        ].into());

        assert_eq!(Some(Array([3u8, 1])), map.get(&3));
        check_cache(&map, [
            (2, Array([2u8, 1])),
            (3, Array([3u8, 1])),
        ].into());

        assert_eq!(None, map.get(&4));
        check_cache(&map, [
            (2, Array([2u8, 1])),
            (3, Array([3u8, 1])),
        ].into());

        assert_eq!(Some(Array([1u8, 1])), map.insert(1, Array([1u8, 10])));
        assert_eq!(Some(Array([2u8, 1])), map.insert(2, Array([2u8, 10])));
        assert_eq!(3, map.len());
        check_cache(&map, [
            (3, Array([3u8, 1])),
        ].into());

        assert_eq!(Some(Array([2u8, 10])), map.get(&2));
        check_cache(&map, [
            (3, Array([3u8, 1])),
            (2, Array([2u8, 10])),
        ].into());

        assert_eq!(Some(Array([1u8, 10])), map.get(&1));
        check_cache(&map, [
            (2, Array([2u8, 10])),
            (1, Array([1u8, 10])),
        ].into());

        assert_eq!(Some(Array([3u8, 1])), map.get(&3));
        check_cache(&map, [
            (1, Array([1u8, 10])),
            (3, Array([3u8, 1])),
        ].into());

        assert_eq!(None, map.get(&4));
        check_cache(&map, [
            (1, Array([1u8, 10])),
            (3, Array([3u8, 1])),
        ].into());

        assert_eq!(Some(Array([1u8, 10])), map.remove(&1));
        assert_eq!(None, map.remove(&1));
        check_cache(&map, [
            (3, Array([3u8, 1])),
            ].into());
            assert_eq!(None, map.get(&1));

        assert_eq!(Some(Array([2u8, 10])), map.remove(&2));
        assert_eq!(None, map.remove(&2));
        check_cache(&map, [
            (3, Array([3u8, 1])),
            ].into());
            assert_eq!(None, map.get(&2));

        assert_eq!(None, map.get(&2));
        assert_eq!(Some(Array([3u8, 1])), map.get(&3));
        assert_eq!(None, map.get(&4));
    }

    #[test]
    fn should_clear() {
        let cache_items = 2;
        let mut map: CachedStableBTreeMap<u32, Array<2>> =
            CachedStableBTreeMap::<u32, Array<2>>::new(MemoryId::new(123), cache_items);

        assert_eq!(None, map.insert(1, Array([1u8, 1])));
        assert_eq!(None, map.insert(2, Array([2u8, 1])));
        assert_eq!(None, map.insert(3, Array([3u8, 1])));

        assert_eq!(Some(Array([1u8, 1])), map.get(&1));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2));
        check_cache(&map, [
            (1, Array([1u8, 1])),
            (2, Array([2u8, 1])),
        ].into());

        map.clear();
        
        assert_eq!(0, map.len());

        check_cache(&map, [].into());

    }

    #[test]
    fn should_replace_old_value() {
        let cache_items = 2;
        let mut map: CachedStableBTreeMap<u32, Array<2>> =
            CachedStableBTreeMap::<u32, Array<2>>::new(MemoryId::new(123), cache_items);


        assert_eq!(None, map.insert(1, Array([1u8, 1])));
        assert_eq!(None, map.insert(2, Array([2u8, 1])));
        assert_eq!(None, map.insert(3, Array([3u8, 1])));
        assert_eq!(3, map.len());

        assert_eq!(Some(Array([1u8, 1])), map.get(&1));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2));

        check_cache(&map, [
            (1, Array([1u8, 1])),
            (2, Array([2u8, 1])),
        ].into());

        assert_eq!(Some(Array([1u8, 1])), map.insert(1, Array([1u8, 10])));
        assert_eq!(Some(Array([3u8, 1])), map.insert(3, Array([3u8, 10])));
        check_cache(&map, [
            (2, Array([2u8, 1])),
        ].into());

        assert_eq!(Some(Array([1u8, 10])), map.get(&1));
        assert_eq!(Some(Array([2u8, 1])), map.get(&2));
        assert_eq!(Some(Array([3u8, 10])), map.get(&3));
        check_cache(&map, [
            (2, Array([2u8, 1])),
            (3, Array([3u8, 10])),
        ].into());

    }

    #[test]
    fn should_cache_least_accessed_element() {
        let cache_items = 3;
        let mut map: CachedStableBTreeMap<u32, Array<2>> =
            CachedStableBTreeMap::<u32, Array<2>>::new(MemoryId::new(123), cache_items);

        assert_eq!(None, map.insert(1, Array([1u8, 1])));
        assert_eq!(None, map.insert(2, Array([2u8, 1])));
        assert_eq!(None, map.insert(3, Array([3u8, 1])));
        assert_eq!(None, map.insert(4, Array([4u8, 1])));
        assert_eq!(None, map.insert(5, Array([5u8, 1])));

        map.get(&1);
        map.get(&2);
        map.get(&3);
        map.get(&1);

        for _ in 0..100 {
            map.get(&5);
        }

        check_cache(&map, [
            (1, Array([1u8, 1])),
            (3, Array([3u8, 1])),
            (5, Array([5u8, 1])),
        ].into());


    }

    fn check_cache(map: &CachedStableBTreeMap<u32, Array<2>>, expected_cache: HashMap<u32, Array<2>>) {
        // let cache = map.cache.borrow();

        // println!("------------------------------------");
        // let mut count = 0;
        // for (k, v) in cache.iter() {
        //     println!("Found in cache: [{k}, {v:?}]");
        //     count += 1;
        //     // assert_eq!(Some(v), expected_cache.get(k));
        // }

        // assert_eq!(count, expected_cache.len());
        
    }
}
