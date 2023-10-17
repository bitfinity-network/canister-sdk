use std::borrow::Cow;
use std::marker::PhantomData;

use dfinity_stable_structures::storable::Bound;
use dfinity_stable_structures::{btreemap, Memory, StableBTreeMap, Storable};

use crate::structure::MultimapStructure;
use crate::Bounds;

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
pub struct StableMultimap<K1, K2, V, M>(StableBTreeMap<KeyPair<K1, K2>, Value<V>, M>)
where
    K1: Storable,
    K2: Storable,
    V: Storable,
    M: Memory;

impl<K1, K2, V, M> StableMultimap<K1, K2, V, M>
where
    K1: Storable,
    K2: Storable,
    V: Storable,
    M: Memory,
{
    /// Create a new instance of a `StableMultimap`.
    /// All keys and values byte representations should be less then related `..._max_size` arguments.
    pub fn new(memory: M) -> Self {
        let _ = KeyPair::<K1, K2>::K1_BOUNDS;
        let _ = KeyPair::<K1, K2>::K2_BOUNDS;
        Self(StableBTreeMap::init(memory))
    }
}

impl<K1, K2, V, M> MultimapStructure<K1, K2, V> for StableMultimap<K1, K2, V, M>
where
    K1: Storable,
    K2: Storable,
    V: Storable,
    M: Memory,
{
    type Iterator<'a> = StableMultimapIter<'a, K1, K2, V, M> where Self: 'a;

    type RangeIterator<'a> = StableMultimapRangeIter<'a, K1, K2, V, M> where Self: 'a;

    fn insert(&mut self, first_key: &K1, second_key: &K2, value: &V) -> Option<V> {
        let key = KeyPair::new(first_key, second_key);
        self.0.insert(key, value.into()).map(|v| v.into_inner())
    }

    fn get(&self, first_key: &K1, second_key: &K2) -> Option<V> {
        let key = KeyPair::new(first_key, second_key);
        self.0.get(&key).map(|v| v.into_inner())
    }

    fn remove(&mut self, first_key: &K1, second_key: &K2) -> Option<V> {
        let key = KeyPair::new(first_key, second_key);

        self.0.remove(&key).map(Value::into_inner)
    }

    fn remove_partial(&mut self, first_key: &K1) -> bool {
        let min_key = KeyPair::<K1, K2>::min_key(first_key);
        let max_key = KeyPair::<K1, K2>::max_key(first_key);

        let keys: Vec<_> = self
            .0
            .range(min_key..=max_key)
            .map(|(keys, _)| keys)
            .collect();

        let mut found = false;
        for k in keys {
            found = self.0.remove(&k).is_some() || found;
        }
        found
    }

    fn len(&self) -> usize {
        self.0.len() as usize
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn clear(&mut self) {
        let keys: Vec<_> = self.0.iter().map(|(k, _)| k).collect();
        for key in keys {
            self.0.remove(&key);
        }
    }

    fn range(&self, first_key: &K1) -> Self::RangeIterator<'_> {
        let min_key = KeyPair::<K1, K2>::min_key(first_key);
        let max_key = KeyPair::<K1, K2>::max_key(first_key);

        let inner = self.0.range(min_key..=max_key);
        StableMultimapRangeIter::new(inner)
    }

    fn iter(&self) -> Self::Iterator<'_> {
        StableMultimapIter::new(self.0.iter())
    }
}

struct KeyPair<K1, K2> {
    encoded: Vec<u8>,
    _p: PhantomData<(K1, K2)>,
}

impl<K1: Storable, K2: Storable> Clone for KeyPair<K1, K2> {
    fn clone(&self) -> Self {
        Self {
            encoded: self.encoded.clone(),
            _p: self._p,
        }
    }
}

impl<K1: Storable, K2: Storable> PartialEq for KeyPair<K1, K2> {
    fn eq(&self, other: &Self) -> bool {
        self.encoded == other.encoded
    }
}

impl<K1: Storable, K2: Storable> Eq for KeyPair<K1, K2> {}

impl<K1: Storable, K2: Storable> PartialOrd for KeyPair<K1, K2> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<K1: Storable, K2: Storable> Ord for KeyPair<K1, K2> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.encoded.cmp(&other.encoded)
    }
}

const fn to_key_bounds<K: Storable>() -> Bounds {
    match K::BOUND {
        Bound::Unbounded => panic!("Multimap keys must be bounded and fixed size"),
        Bound::Bounded {
            max_size,
            is_fixed_size,
        } => {
            if !is_fixed_size {
                panic!("Multimap keys must be bounded and fixed size")
            }
            Bounds::new(max_size as usize, is_fixed_size)
        }
    }
}

impl<K1, K2> KeyPair<K1, K2>
where
    K1: Storable,
    K2: Storable,
{
    const K1_BOUNDS: Bounds = to_key_bounds::<K1>();
    const K2_BOUNDS: Bounds = to_key_bounds::<K2>();

    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `second_key.to_bytes().len() <= K2::MAX_SIZE`
    pub fn new(first_key: &K1, second_key: &K2) -> Self {
        let first_key_bytes = first_key.to_bytes();
        let second_key_bytes = second_key.to_bytes();

        assert!(first_key_bytes.len() <= Self::K1_BOUNDS.max_size);
        assert!(second_key_bytes.len() <= Self::K2_BOUNDS.max_size);

        let full_len = Self::K1_BOUNDS.max_size + Self::K2_BOUNDS.max_size;
        let mut buffer = Vec::with_capacity(full_len);
        buffer.extend_from_slice(&first_key_bytes);
        buffer.extend_from_slice(&second_key_bytes);

        Self {
            encoded: buffer,
            _p: PhantomData,
        }
    }

    pub fn first_key(&self) -> K1 {
        K1::from_bytes(self.encoded[..Self::K1_BOUNDS.max_size].into())
    }

    pub fn second_key(&self) -> K2 {
        K2::from_bytes(self.encoded[Self::K1_BOUNDS.max_size..].into())
    }

    /// Minimum possible `KeyPair` for the specified `first_key`.
    pub fn min_key(first_key: &K1) -> Self {
        let mut first_key_bytes = first_key.to_bytes().to_vec();

        assert!(first_key_bytes.len() == Self::K1_BOUNDS.max_size);

        first_key_bytes.resize(Self::K1_BOUNDS.max_size + Self::K2_BOUNDS.max_size, 0x0);

        Self {
            encoded: first_key_bytes,
            _p: PhantomData,
        }
    }

    /// Maximum possible `KeyPair` for the specified `first_key`.
    pub fn max_key(first_key: &K1) -> Self {
        let mut first_key_bytes = first_key.to_bytes().to_vec();

        assert!(first_key_bytes.len() == Self::K1_BOUNDS.max_size);

        first_key_bytes.resize(Self::K1_BOUNDS.max_size + Self::K2_BOUNDS.max_size, 0xFF);

        Self {
            encoded: first_key_bytes,
            _p: PhantomData,
        }
    }
}

impl<K1, K2> Storable for KeyPair<K1, K2>
where
    K1: Storable,
    K2: Storable,
{
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(&self.encoded)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self {
            encoded: bytes.to_vec(),
            _p: PhantomData,
        }
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: (Self::K1_BOUNDS.max_size + Self::K2_BOUNDS.max_size) as u32,
        is_fixed_size: true,
    };
}

struct Value<V>(Vec<u8>, PhantomData<V>);

impl<V: Storable> Value<V> {
    pub fn into_inner(self) -> V {
        V::from_bytes(self.0.into())
    }
}

impl<V: Storable> From<&V> for Value<V> {
    fn from(value: &V) -> Self {
        Self(value.to_bytes().into(), PhantomData)
    }
}

impl<V: Storable> Storable for Value<V> {
    const BOUND: dfinity_stable_structures::storable::Bound = V::BOUND;

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(&self.0)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self(bytes.to_vec(), PhantomData)
    }
}

/// Range iterator
pub struct StableMultimapRangeIter<'a, K1, K2, V, M>
where
    K1: Storable,
    K2: Storable,
    V: Storable,
    M: Memory,
{
    inner: btreemap::Iter<'a, KeyPair<K1, K2>, Value<V>, M>,
}

impl<'a, K1, K2, V, M> StableMultimapRangeIter<'a, K1, K2, V, M>
where
    K1: Storable,
    K2: Storable,
    V: Storable,
    M: Memory,
{
    fn new(inner: btreemap::Iter<'a, KeyPair<K1, K2>, Value<V>, M>) -> Self {
        Self { inner }
    }
}

// -----------------------------------------------------------------------------
//     - Range Iterator impl -
// -----------------------------------------------------------------------------
impl<'a, K1, K2, V, M> Iterator for StableMultimapRangeIter<'a, K1, K2, V, M>
where
    K1: Storable,
    K2: Storable,
    V: Storable,
    M: Memory,
{
    type Item = (K2, V);

    fn next(&mut self) -> Option<(K2, V)> {
        self.inner
            .next()
            .map(|(keys, v)| (keys.second_key(), v.into_inner()))
    }
}

pub struct StableMultimapIter<'a, K1, K2, V, M>(btreemap::Iter<'a, KeyPair<K1, K2>, Value<V>, M>)
where
    K1: Storable,
    K2: Storable,
    V: Storable,
    M: Memory;

impl<'a, K1, K2, V, M> StableMultimapIter<'a, K1, K2, V, M>
where
    K1: Storable,
    K2: Storable,
    V: Storable,
    M: Memory,
{
    fn new(inner: btreemap::Iter<'a, KeyPair<K1, K2>, Value<V>, M>) -> Self {
        Self(inner)
    }
}

impl<'a, K1, K2, V, M> Iterator for StableMultimapIter<'a, K1, K2, V, M>
where
    K1: Storable,
    K2: Storable,
    V: Storable,
    M: Memory,
{
    type Item = (K1, K2, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(keys, val)| {
            let k1 = keys.first_key();
            let k2 = keys.second_key();
            (k1, k2, val.into_inner())
        })
    }
}

impl<'a, K1, K2, V, M> IntoIterator for &'a StableMultimap<K1, K2, V, M>
where
    K1: Storable,
    K2: Storable,
    V: Storable,
    M: Memory,
{
    type Item = (K1, K2, V);

    type IntoIter = StableMultimapIter<'a, K1, K2, V, M>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod test {

    use dfinity_stable_structures::VectorMemory;

    use super::*;
    use crate::test_utils::{Array, StringValue};

    fn make_map() -> StableMultimap<Array<2>, Array<3>, Array<6>, VectorMemory> {
        let mut mm = StableMultimap::new(VectorMemory::default());
        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = Array([200u8, 200, 200, 100, 100, 123]);
        mm.insert(&k1, &k2, &val);

        let k1 = Array([10u8, 20]);
        let k2 = Array([21u8, 22, 23]);
        let val = Array([123, 200u8, 200, 100, 100, 255]);
        mm.insert(&k1, &k2, &val);

        mm
    }

    #[test]
    fn inserts() {
        let mut mm: StableMultimap<Array<1>, Array<2>, Array<1>, _> =
            StableMultimap::new(VectorMemory::default());
        for i in 0..10 {
            let k1 = Array([i; 1]);
            let k2 = Array([i * 10; 2]);
            let val = Array([i; 1]);
            mm.insert(&k1, &k2, &val);
        }

        assert_eq!(mm.len(), 10);
    }

    #[test]
    fn insert_should_replace_old_value() {
        let mut mm = make_map();

        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = Array([255u8, 255, 255, 255, 255, 255]);

        let prev_val = Array([200u8, 200, 200, 100, 100, 123]);
        let replaced_val = mm.insert(&k1, &k2, &val).unwrap();

        assert_eq!(prev_val, replaced_val);
        assert_eq!(mm.get(&k1, &k2), Some(val));
    }

    #[test]
    fn get() {
        let mm = make_map();
        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = mm.get(&k1, &k2).unwrap();

        let expected = Array([200u8, 200, 200, 100, 100, 123]);
        assert_eq!(val, expected);
    }

    #[test]
    fn remove() {
        let mut mm = make_map();
        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = mm.remove(&k1, &k2).unwrap();

        let expected = Array([200u8, 200, 200, 100, 100, 123]);
        assert_eq!(val, expected);
        assert_eq!(mm.len(), 1);

        let k1 = Array([10u8, 20]);
        let k2 = Array([21u8, 22, 23]);
        mm.remove(&k1, &k2).unwrap();
        assert!(mm.is_empty());
    }

    #[test]
    fn remove_partial() {
        let mut mm = StableMultimap::new(VectorMemory::default());
        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = Array([200u8, 200, 200, 100, 100, 123]);
        mm.insert(&k1, &k2, &val);

        let k2 = Array([21u8, 22, 23]);
        let val = Array([123, 200u8, 200, 100, 100, 255]);
        mm.insert(&k1, &k2, &val);

        assert!(mm.remove_partial(&k1));
        assert!(!mm.remove_partial(&k1));
        assert!(mm.is_empty());
    }

    #[test]
    fn clear() {
        let mut mm = StableMultimap::new(VectorMemory::default());
        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = Array([200u8, 200, 200, 100, 100, 123]);
        mm.insert(&k1, &k2, &val);

        let k2 = Array([21u8, 22, 23]);
        let val = Array([123, 200u8, 200, 100, 100, 255]);
        mm.insert(&k1, &k2, &val);
        let k1 = Array([21u8, 22]);
        mm.insert(&k1, &k2, &val);

        mm.clear();
        assert!(mm.is_empty());
    }

    #[test]
    fn iter() {
        let mm = make_map();
        let mut iter = mm.into_iter();
        assert!(iter.next().is_some());
        assert!(iter.next().is_some());
        assert!(iter.next().is_none());
    }

    #[test]
    fn range_iter() {
        let k1 = Array([1u8, 2]);
        let mm = make_map();
        let mut iter = mm.range(&k1);
        assert!(iter.next().is_some());
        assert!(iter.next().is_none());
    }

    #[test]
    fn multimap_works() {
        let mut map = StableMultimap::new(VectorMemory::default());
        assert!(map.is_empty());

        map.insert(&0u32, &0u32, &42u32);
        map.insert(&0u32, &1u32, &84u32);

        map.insert(&1u32, &0u32, &10u32);
        map.insert(&1u32, &1u32, &20u32);

        assert_eq!(map.len(), 4);
        assert_eq!(map.get(&0, &0), Some(42));
        assert_eq!(map.get(&0, &1), Some(84));
        assert_eq!(map.get(&1, &0), Some(10));
        assert_eq!(map.get(&1, &1), Some(20));

        let mut iter = map.iter();
        assert_eq!(iter.next(), Some((0, 0, 42)));
        assert_eq!(iter.next(), Some((0, 1, 84)));
        assert_eq!(iter.next(), Some((1, 0, 10)));
        assert_eq!(iter.next(), Some((1, 1, 20)));
        assert_eq!(iter.next(), None);

        let mut range = map.range(&0);
        assert_eq!(range.next(), Some((0, 42)));
        assert_eq!(range.next(), Some((1, 84)));
        assert_eq!(range.next(), None);

        assert!(map.remove_partial(&0));
        assert!(!map.remove_partial(&0));
        assert_eq!(map.len(), 2);

        assert_eq!(map.remove(&1, &0), Some(10));
        assert_eq!(map.iter().next(), Some((1, 1, 20)));
        assert_eq!(map.len(), 1);
    }

    #[should_panic]
    #[test]
    fn btreemap_should_not_allow_undounded_key_1() {
        let _: StableMultimap<StringValue, u32, u32, _> =
            StableMultimap::new(VectorMemory::default());
    }

    #[should_panic]
    #[test]
    fn btreemap_should_not_allow_undounded_key_2() {
        let _: StableMultimap<u32, StringValue, u32, _> =
            StableMultimap::new(VectorMemory::default());
    }
}
