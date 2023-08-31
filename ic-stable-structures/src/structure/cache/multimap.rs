use std::cell::RefCell;
use std::collections::VecDeque;
use std::hash::Hash;

use ic_exports::stable_structures::BoundedStorable;

use crate::structure::heap;
use crate::structure::stable_storage::StableMultimap;

// Keys memory layout:
//
// |- k1 size in bytes -|- k1 bytes -|- k2 bytes |
//
// Size of k1 is stored because we need to make a difference between
// a k1 bytes and another shorter k1 bytes + k2 start bytes.
// For example, we have two key pairs with byte patterns:
// 1) k1 = [0x1, 0x2, 0x3] and k2 = [0x4, 0x5]
// 2) k1 = [0x1, 0x2] and k2 = [0x3, 0x4, 0x5]
//
// Concatination of both key pairs is same: [0x1, 0x2, 0x3, 0x4, 0x5],
// but with the `k1 size` prefix, it is different:
// 1) [0x3, 0x1, 0x2, 0x3, 0x4, 0x5]
// 2) [0x2, 0x1, 0x2, 0x3, 0x4, 0x5]
//
// Bytes count of `k1 size` is calculated from the `first_key_max_size` (see `size_bytes_len()`). Usually,
// keys are shorter then 256 bytes, so, size overhead will be just one byte per value.
// Inner [`StableBTreeMap`] limits max size by `u32::MAX`, so in worst case
// (for keys with max size greater then 65535), we will spend four bytes per value.

/// `StableMultimap` stores two keys against a single value, making it possible
/// to fetch all values by the root key, or a single value by specifying both keys.
pub struct CachedStableMultimap<K1, K2, V>
where
    K1: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    K2: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    inner: StableMultimap<K1, K2, V>,
    cache: RefCell<Cache<K1, K2, V>>,
}

struct Cache<K1, K2, V>
where
    K1: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    K2: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    cache: heap::HeapMultimap<K1, K2, V>,
    cache_keys: VecDeque<(K1, K2)>,
    cache_max_items: usize,
}

impl<K1, K2, V> CachedStableMultimap<K1, K2, V>
where
    K1: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    K2: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    /// Create a new instance of a `StableMultimap`.
    /// All keys and values byte representations should be less then related `..._max_size` arguments.
    pub fn new(inner: StableMultimap<K1, K2, V>, cache_max_items: usize) -> Self {
        Self {
            inner,
            cache: RefCell::new(Cache {
                cache_max_items,
                cache: Default::default(),
                cache_keys: Default::default(),
            }),
        }
    }

    /// Insert a new value into the map.
    /// Inserting a value with the same keys as an existing value
    /// will result in the old value being overwritten.
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `second_key.to_bytes().len() <= K2::MAX_SIZE`
    ///   - `value.to_bytes().len() <= V::MAX_SIZE`
    pub fn insert(&mut self, first_key: &K1, second_key: &K2, value: &V) -> Option<V> {
        self.inner.insert(first_key, second_key, value)
    }

    /// Get a value for the given keys.
    /// If byte representation length of any key exceeds max size, `None` will be returned.
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `second_key.to_bytes().len() <= K2::MAX_SIZE`
    pub fn get(&self, first_key: &K1, second_key: &K2) -> Option<V> {
        let cache = self.cache.borrow();
        match cache.cache.get(first_key, second_key) {
            Some(value) => Some(value),
            None => {
                drop(cache);
                match self.inner.get(first_key, second_key) {
                    Some(value) => {
                        {
                            let mut cache = self.cache.borrow_mut();
                            cache.cache.insert(first_key, second_key, &value);
                            cache
                                .cache_keys
                                .push_back((first_key.clone(), second_key.clone()));
                            self.remove_oldest_from_cache(&mut cache);
                        }
                        Some(value)
                    }
                    None => None,
                }
            }
        }
    }

    #[inline]
    fn remove_oldest_from_cache(&self, cache: &mut Cache<K1, K2, V>) {
        if cache.cache_keys.len() > cache.cache_max_items {
            if let Some((k1, k2)) = cache.cache_keys.pop_front() {
                cache.cache.remove(&k1, &k2);
            };
        }
    }
    /// Remove a specific value and return it.
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `second_key.to_bytes().len() <= K2::MAX_SIZE`
    pub fn remove(&mut self, first_key: &K1, second_key: &K2) -> Option<V> {
        {
            let mut cache = self.cache.borrow_mut();
            if cache.cache.remove(first_key, second_key).is_some() {
                if let Some(pos) = cache
                    .cache_keys
                    .iter()
                    .position(|(k1, k2)| k1 == first_key && k2 == second_key)
                {
                    cache.cache_keys.remove(pos);
                }
            }
        }
        self.inner.remove(first_key, second_key)
    }

    /// Remove all values for the partial key
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    pub fn remove_partial(&mut self, first_key: &K1) -> bool {
        {
            let mut cache = self.cache.borrow_mut();
            if cache.cache.remove_partial(first_key) {
                cache.cache_keys.retain(|(k1, _k2)| k1 != first_key);
            }
        }
        self.inner.remove_partial(first_key)
    }

    // /// Get a range of key value pairs based on the root key.
    // ///
    // /// # Preconditions:
    // ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    // pub fn range(&self, first_key: &K1) -> RangeIter<MemoryId, K1, K2, V> {
    //     self.inner.range(first_key)
    // }

    // /// Iterator over all items in the map.
    // pub fn iter(&self) -> Iter<MemoryId, K1, K2, V> {
    //     self.inner.iter()
    // }

    /// Item count.
    pub fn len(&self) -> usize {
        self.inner.len() as usize
    }

    /// Is the map empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear(&mut self) {
        {
            let mut cache = self.cache.borrow_mut();
            cache.cache.clear();
            cache.cache_keys.clear();
        }
        self.inner.clear()
    }
}

#[cfg(test)]
mod test {
    use std::borrow::Cow;

    use ic_exports::stable_structures::{memory_manager::MemoryId, Storable};

    use super::*;

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

    // fn make_map() -> CachedStableMultimap<DefaultMemoryImpl, Array<2>, Array<3>, Array<6>> {
    //     let mut mm = CachedStableMultimap::new(DefaultMemoryImpl::default());
    //     let k1 = Array([1u8, 2]);
    //     let k2 = Array([11u8, 12, 13]);
    //     let val = Array([200u8, 200, 200, 100, 100, 123]);
    //     mm.insert(&k1, &k2, &val);

    //     let k1 = Array([10u8, 20]);
    //     let k2 = Array([21u8, 22, 23]);
    //     let val = Array([123, 200u8, 200, 100, 100, 255]);
    //     mm.insert(&k1, &k2, &val);

    //     mm
    // }

    #[test]
    fn should_get_and_insert() {
        let cache_items = 2;
        let mut map = CachedStableMultimap::<u32, u32, Array<2>>::new(
            StableMultimap::new(MemoryId::new(123)),
            cache_items,
        );

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

    //     #[test]
    //     fn inserts() {
    //         let mut mm = CachedStableMultimap::new(DefaultMemoryImpl::default());
    //         for i in 0..10 {
    //             let k1 = Array([i; 1]);
    //             let k2 = Array([i * 10; 2]);
    //             let val = Array([i; 1]);
    //             mm.insert(&k1, &k2, &val);
    //         }

    //         assert_eq!(mm.len(), 10);
    //     }

    //     #[test]
    //     fn insert_should_replace_old_value() {
    //         let mut mm = make_map();

    //         let k1 = Array([1u8, 2]);
    //         let k2 = Array([11u8, 12, 13]);
    //         let val = Array([255u8, 255, 255, 255, 255, 255]);

    //         let prev_val = Array([200u8, 200, 200, 100, 100, 123]);
    //         let replaced_val = mm.insert(&k1, &k2, &val).unwrap();

    //         assert_eq!(prev_val, replaced_val);
    //         assert_eq!(mm.get(&k1, &k2), Some(val));
    //     }

    //     #[test]
    //     fn get() {
    //         let mm = make_map();
    //         let k1 = Array([1u8, 2]);
    //         let k2 = Array([11u8, 12, 13]);
    //         let val = mm.get(&k1, &k2).unwrap();

    //         let expected = Array([200u8, 200, 200, 100, 100, 123]);
    //         assert_eq!(val, expected);
    //     }

    //     #[test]
    //     fn remove() {
    //         let mut mm = make_map();
    //         let k1 = Array([1u8, 2]);
    //         let k2 = Array([11u8, 12, 13]);
    //         let val = mm.remove(&k1, &k2).unwrap();

    //         let expected = Array([200u8, 200, 200, 100, 100, 123]);
    //         assert_eq!(val, expected);
    //         assert_eq!(mm.len(), 1);

    //         let k1 = Array([10u8, 20]);
    //         let k2 = Array([21u8, 22, 23]);
    //         mm.remove(&k1, &k2).unwrap();
    //         assert!(mm.is_empty());
    //     }

    //     #[test]
    //     fn remove_partial() {
    //         let mut mm = CachedStableMultimap::new(DefaultMemoryImpl::default());
    //         let k1 = Array([1u8, 2]);
    //         let k2 = Array([11u8, 12, 13]);
    //         let val = Array([200u8, 200, 200, 100, 100, 123]);
    //         mm.insert(&k1, &k2, &val);

    //         let k2 = Array([21u8, 22, 23]);
    //         let val = Array([123, 200u8, 200, 100, 100, 255]);
    //         mm.insert(&k1, &k2, &val);

    //         mm.remove_partial(&k1);
    //         assert!(mm.is_empty());
    //     }

    //     #[test]
    //     fn clear() {
    //         let mut mm = CachedStableMultimap::new(DefaultMemoryImpl::default());
    //         let k1 = Array([1u8, 2]);
    //         let k2 = Array([11u8, 12, 13]);
    //         let val = Array([200u8, 200, 200, 100, 100, 123]);
    //         mm.insert(&k1, &k2, &val);

    //         let k2 = Array([21u8, 22, 23]);
    //         let val = Array([123, 200u8, 200, 100, 100, 255]);
    //         mm.insert(&k1, &k2, &val);
    //         let k1 = Array([21u8, 22]);
    //         mm.insert(&k1, &k2, &val);

    //         mm.clear();
    //         assert!(mm.is_empty());
    //     }

    //     #[test]
    //     fn iter() {
    //         let mm = make_map();
    //         let mut iter = mm.into_iter();
    //         assert!(iter.next().is_some());
    //         assert!(iter.next().is_some());
    //         assert!(iter.next().is_none());
    //     }

    //     #[test]
    //     fn range_iter() {
    //         let k1 = Array([1u8, 2]);
    //         let mm = make_map();
    //         let mut iter = mm.range(&k1);
    //         assert!(iter.next().is_some());
    //         assert!(iter.next().is_none());
    //     }
}
