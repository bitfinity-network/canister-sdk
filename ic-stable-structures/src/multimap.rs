use std::marker::PhantomData;

use ic_exports::stable_structures::{btreemap, Memory, StableBTreeMap, Storable};

use crate::Error;

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
// Inner [`StableBTreeMap`] limits max suze by `u32::MAX`, so in worst case
// (for keys with max size greater then 65535), we will spend four bytes per value.

/// [`StableMultimap`] stores two keys against a single value, making it possible
/// to fetch all values by the root key, or a single value by specifying both keys.
/// ```
/// use ic_stable_storage::StableMultimap;
/// let mut map = StableMultimap::<_, _, _, 0>::new().unwrap();
/// // Same root key of 1
/// map.insert(1, 1, 1);
/// map.insert(1, 2, 2);
/// // Different root key of 4
/// map.insert(4, 2, 2);
///
/// assert_eq!(map.range(1).unwrap().count(), 2);
/// assert_eq!(map.range(4).unwrap().count(), 1);
/// ```
pub struct StableMultimap<M: Memory + Clone, K1, K2, V> {
    _p: PhantomData<(K1, K2, V)>,
    inner: StableBTreeMap<M, Vec<u8>, Vec<u8>>,
    first_key_max_size: u32,
    second_key_max_size: u32,
}

impl<M, K1, K2, V> StableMultimap<M, K1, K2, V>
where
    M: Memory + Clone,
    K1: Storable,
    K2: Storable,
    V: Storable,
{
    /// Create a new instance of a [`StableMultimap`].
    /// Note that all keys and values has to implement both [`Default`] and [`Copy`] as the keys
    /// and value lenghts are calculated when the map is created.
    pub fn new(
        memory: M,
        first_key_max_size: u32,
        second_key_max_size: u32,
        value_max_size: u32,
    ) -> Self {
        let both_keys_max_size = first_key_max_size + second_key_max_size;
        let first_key_size_bytes_len = size_bytes_len(first_key_max_size);
        let inner_key_max_size = both_keys_max_size + first_key_size_bytes_len as u32;

        let inner = StableBTreeMap::init(memory, inner_key_max_size, value_max_size);

        Self {
            _p: PhantomData,
            inner,
            first_key_max_size,
            second_key_max_size,
        }
    }

    /// Insert a new value into the map.
    /// Inserting a value with the same keys as an existing value
    /// will result in the old value being overwritten.
    pub fn insert(&mut self, first_key: &K1, second_key: &K2, value: V) -> Result<(), Error> {
        let keys_bytes = self.both_keys_bytes(first_key, second_key)?;
        let value = value.to_bytes();
        self.inner.insert(keys_bytes, value.to_vec())?;
        Ok(())
    }

    /// Get a value for the given keys
    pub fn get(&self, first_key: &K1, second_key: &K2) -> Option<V> {
        let keys_bytes = self.both_keys_bytes(first_key, second_key).ok()?;

        let bytes = self.inner.get(&keys_bytes)?;
        Some(V::from_bytes(bytes))
    }

    /// Remove a specific value and return it
    /// ```
    /// use ic_stable_storage::StableMultimap;
    /// let mut map = StableMultimap::<_, _, _, 0>::new().unwrap();
    /// map.insert(1, 2, 3);
    /// assert_eq!(map.remove(&1, &2).unwrap(), 3);
    /// ```
    pub fn remove(&mut self, first_key: &K1, second_key: &K2) -> Result<Option<V>, Error> {
        let keys_bytes = self.both_keys_bytes(first_key, second_key)?;

        let value = self.inner.remove(&keys_bytes).map(V::from_bytes);
        Ok(value)
    }

    /// Remove all values for the partial key
    /// ```
    /// use ic_stable_storage::StableMultimap;
    /// let mut map = StableMultimap::<_, _, _, 0>::new().unwrap();
    /// // Same root key of 1
    /// map.insert(1, 2, 3);
    /// map.insert(1, 3, 4);
    /// // Separate root key, will not be removed.
    /// map.insert(2, 2, 1);
    /// map.remove_partial(&1).unwrap();
    /// assert_eq!(map.len(), 1);
    /// ```
    pub fn remove_partial(&mut self, first_key: &K1) -> Result<(), Error> {
        let key_prefix = self.first_key_bytes(first_key)?;

        let keys = self
            .inner
            .range(key_prefix, None)
            .map(|(k1k2, _)| k1k2)
            .collect::<Vec<_>>();

        for k in keys {
            let _ = self.inner.remove(&k);
        }

        Ok(())
    }

    /// Get a range of key value pairs based on the root key.
    /// ```
    /// use ic_stable_storage::StableMultimap;
    /// let mut map = StableMultimap::<_, _, _, 0>::new().unwrap();
    /// // Same root key of 1
    /// map.insert(1, 2, 3);
    /// map.insert(1, 3, 4);
    /// // Separate root key, will not be included
    /// map.insert(2, 2, 1);
    /// let iter = map.range(1).unwrap();
    /// assert_eq!(iter.count(), 2);
    /// ```
    pub fn range(&self, first_key: &K1) -> Result<RangeIter<M, K2, V>, Error> {
        let key_prefix = self.first_key_bytes(first_key)?;

        let inner = self.inner.range(key_prefix, None);
        let iter = RangeIter::new(inner, self.first_key_max_size);

        Ok(iter)
    }

    /// Iterator over all items in map.
    pub fn iter(&self) -> Iter<M, K1, K2, V> {
        Iter::new(self.inner.iter(), self.first_key_max_size)
    }

    /// Items count.
    pub fn len(&self) -> usize {
        self.inner.len() as usize
    }

    /// Is map empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn first_key_bytes(&self, first_key: &K1) -> Result<Vec<u8>, Error> {
        let mut buf = Vec::with_capacity(self.first_key_with_size_prefix_max_bytes_len());
        Self::push_bytes_with_size_prefix(first_key, self.first_key_max_size, &mut buf)?;
        Ok(buf)
    }

    fn both_keys_bytes(&self, first_key: &K1, second_key: &K2) -> Result<Vec<u8>, Error> {
        let mut buf = Vec::with_capacity(self.both_keys_with_size_prefix_max_bytes_len());
        Self::push_bytes_with_size_prefix(first_key, self.first_key_max_size, &mut buf)?;
        buf.extend_from_slice(&second_key.to_bytes());
        Ok(buf)
    }

    fn first_key_with_size_prefix_max_bytes_len(&self) -> usize {
        size_bytes_len(self.first_key_max_size) + self.first_key_max_size as usize
    }

    fn both_keys_with_size_prefix_max_bytes_len(&self) -> usize {
        self.first_key_with_size_prefix_max_bytes_len() + self.second_key_max_size as usize
    }

    fn push_bytes_with_size_prefix(
        data: &impl Storable,
        max_bytes_len: u32,
        buf: &mut Vec<u8>,
    ) -> Result<(), Error> {
        let data_bytes = data.to_bytes();
        let data_len = data_bytes.len();

        if data_len > max_bytes_len as usize {
            return Err(Error::ValueTooLarge(data_len as _));
        }

        let size_bytes_len = size_bytes_len(max_bytes_len);

        // First, write size of data.
        buf.extend_from_slice(&data_len.to_le_bytes()[..size_bytes_len]);
        // Then, write the data itself.
        buf.extend_from_slice(&data_bytes);

        Ok(())
    }
}

fn size_bytes_len(max_size: u32) -> usize {
    const U8_MAX: u32 = u8::MAX as u32;
    const U16_MAX: u32 = u16::MAX as u32;

    match max_size {
        0..=U8_MAX => 1,
        0..=U16_MAX => 2,
        _ => 4,
    }
}

fn key_size_from_bytes(bytes: &[u8], size_bytes_len: usize) -> usize {
    let mut size_bytes = [0u8; 4];
    size_bytes[..size_bytes_len].copy_from_slice(&bytes[..size_bytes_len]);
    u32::from_le_bytes(size_bytes) as _
}

/// Range iterator
pub struct RangeIter<'a, M: Memory + Clone, K2, V> {
    inner: btreemap::Iter<'a, M, Vec<u8>, Vec<u8>>,
    first_key_max_size: u32,
    _p: PhantomData<(K2, V)>,
}

impl<'a, M, K2, V> RangeIter<'a, M, K2, V>
where
    M: Memory + Clone,
    K2: Storable,
    V: Storable,
{
    pub fn new(inner: btreemap::Iter<'a, M, Vec<u8>, Vec<u8>>, first_key_max_size: u32) -> Self {
        Self {
            inner,
            first_key_max_size,
            _p: PhantomData,
        }
    }

    pub fn second_key_from_both_keys_bytes(&self, both_bytes: &[u8]) -> K2 {
        let first_key_size_bytes_len = size_bytes_len(self.first_key_max_size);
        let first_key_size = key_size_from_bytes(both_bytes, first_key_size_bytes_len);

        let second_key_offset = first_key_size + first_key_size_bytes_len;
        let second_key_bytes = &both_bytes[second_key_offset..];

        K2::from_bytes(second_key_bytes.to_vec())
    }
}

// -----------------------------------------------------------------------------
//     - Range Iterator impl -
// -----------------------------------------------------------------------------
impl<'a, M, K2, V> Iterator for RangeIter<'a, M, K2, V>
where
    M: Memory + Clone,
    K2: Storable,
    V: Storable,
{
    type Item = (K2, V);

    fn next(&mut self) -> Option<(K2, V)> {
        self.inner.next().and_then(|(k1k2, v)| {
            let k2 = self.second_key_from_both_keys_bytes(&k1k2);
            let val = V::from_bytes(v);
            Some((k2, val))
        })
    }
}

pub struct Iter<'a, M: Memory + Clone, K1, K2, V> {
    inner: btreemap::Iter<'a, M, Vec<u8>, Vec<u8>>,
    first_key_max_size: u32,
    _p: PhantomData<(K1, K2, V)>,
}

impl<'a, M, K1, K2, V> Iter<'a, M, K1, K2, V>
where
    M: Memory + Clone,
    K1: Storable,
    K2: Storable,
    V: Storable,
{
    pub fn new(inner: btreemap::Iter<'a, M, Vec<u8>, Vec<u8>>, first_key_max_size: u32) -> Self {
        Self {
            inner,
            first_key_max_size,
            _p: PhantomData,
        }
    }

    pub fn keys_from_bytes(&self, both_bytes: &[u8]) -> (K1, K2) {
        let first_key_size_bytes_len = size_bytes_len(self.first_key_max_size);
        let first_key_size = key_size_from_bytes(both_bytes, first_key_size_bytes_len);
        let second_key_offset = first_key_size + first_key_size_bytes_len;
        let first_key_bytes = &both_bytes[first_key_size_bytes_len..second_key_offset];
        let first_key = K1::from_bytes(first_key_bytes.to_vec());

        let second_key_bytes = &both_bytes[second_key_offset..];
        let second_key = K2::from_bytes(second_key_bytes.to_vec());

        (first_key, second_key)
    }
}

impl<'a, M, K1, K2, V> Iterator for Iter<'a, M, K1, K2, V>
where
    M: Memory + Clone,
    K1: Storable,
    K2: Storable,
    V: Storable,
{
    type Item = (K1, K2, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k1k2, val)| {
            let (k1, k2) = self.keys_from_bytes(&k1k2);
            (k1, k2, V::from_bytes(val))
        })
    }
}

impl<'a, M, K1, K2, V> IntoIterator for &'a StableMultimap<M, K1, K2, V>
where
    M: Memory + Clone,
    K1: Storable,
    K2: Storable,
    V: Storable,
{
    type Item = (K1, K2, V);

    type IntoIter = Iter<'a, M, K1, K2, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod test {
    use std::borrow::Cow;

    use ic_exports::stable_structures::DefaultMemoryImpl;

    use super::*;

    /// New type pattern used to implement `Storable` trait for all arrays.
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    struct Array<const N: usize>(pub [u8; N]);

    impl<const N: usize> Storable for Array<N> {
        fn to_bytes(&self) -> Cow<[u8]> {
            Cow::Owned(self.0.to_vec())
        }

        fn from_bytes(bytes: Vec<u8>) -> Self {
            let mut buf = [0u8; N];
            buf.copy_from_slice(&bytes);
            Array(buf)
        }
    }

    fn make_map() -> StableMultimap<DefaultMemoryImpl, Array<2>, Array<3>, Array<6>> {
        let mut mm = StableMultimap::new(DefaultMemoryImpl::default(), 2, 3, 6);
        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = Array([200u8, 200, 200, 100, 100, 123]);
        mm.insert(&k1, &k2, val).unwrap();

        let k1 = Array([10u8, 20]);
        let k2 = Array([21u8, 22, 23]);
        let val = Array([123, 200u8, 200, 100, 100, 255]);
        mm.insert(&k1, &k2, val).unwrap();

        mm
    }

    #[test]
    fn inserts() {
        let mut mm = StableMultimap::new(DefaultMemoryImpl::default(), 1, 2, 1);
        for i in 0..10 {
            let k1 = Array([i; 1]);
            let k2 = Array([i * 10; 2]);
            let val = Array([i; 1]);
            mm.insert(&k1, &k2, val).unwrap();
        }

        assert_eq!(mm.len(), 10);
    }

    #[test]
    fn insert_overwrites() {
        let mut mm = make_map();
        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = Array([3u8, 0, 0, 0, 0, 3]);
        mm.insert(&k1, &k2, val).unwrap();

        let ret = mm.get(&k1, &k2).unwrap();

        assert_eq!(val, ret);
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
        let val = mm.remove(&k1, &k2).unwrap().unwrap();

        let expected = Array([200u8, 200, 200, 100, 100, 123]);
        assert_eq!(val, expected);
        assert_eq!(mm.len(), 1);

        let k1 = Array([10u8, 20]);
        let k2 = Array([21u8, 22, 23]);
        let _ = mm.remove(&k1, &k2).unwrap();
        assert!(mm.is_empty());
    }

    #[test]
    fn remove_partial() {
        let mut mm = StableMultimap::new(DefaultMemoryImpl::default(), 2, 3, 6);
        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = Array([200u8, 200, 200, 100, 100, 123]);
        mm.insert(&k1, &k2, val).unwrap();

        let k2 = Array([21u8, 22, 23]);
        let val = Array([123, 200u8, 200, 100, 100, 255]);
        mm.insert(&k1, &k2, val).unwrap();

        mm.remove_partial(&k1).unwrap();
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
        let mut iter = mm.range(&k1).unwrap();
        assert!(iter.next().is_some());
        assert!(iter.next().is_none());
    }
}
