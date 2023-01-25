use std::borrow::Cow;
use std::iter::Peekable;
use std::marker::PhantomData;
use std::mem;

use ic_exports::stable_structures::{btreemap, BoundedStorable, Memory, StableBTreeMap, Storable};

pub type ChunkSize = u16;

type ChunkIndex = u16;

const CHUNK_INDEX_LEN: usize = mem::size_of::<ChunkIndex>();

/// Map that allows to store values with arbitrary size in stable memory.
///
/// Current implementation stores values in chunks with fixed size.
/// Size of chunk should be set using the [`SlicedStorable`] trait.
pub struct StableUnboundedMap<M, K, V>
where
    M: Memory + Clone,
    K: BoundedStorable,
    V: SlicedStorable,
{
    inner: StableBTreeMap<Key<K>, Chunk<V>, M>,
    items_count: u64,
}

impl<M, K, V> StableUnboundedMap<M, K, V>
where
    M: Memory + Clone,
    K: BoundedStorable,
    V: SlicedStorable,
{
    /// Create new instance of the map.
    ///
    /// If the `memory` contains data of the map, the map reads it, and the instance
    /// will contain the data from the `memory`.
    pub fn new(memory: M) -> Self {
        Self {
            inner: StableBTreeMap::init(memory),
            items_count: 0,
        }
    }

    /// Return a value associated with `key`.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn get(&self, key: &K) -> Option<V> {
        let first_chunk_key = Key::new(key);
        let max_chunk_key = first_chunk_key.clone().with_max_chunk_index();
        let mut value_data = Vec::new();
        let mut item_present = false;

        for (_, chunk) in self.inner.range(first_chunk_key..=max_chunk_key) {
            value_data.extend_from_slice(&chunk.to_bytes());
            item_present = true;
        }

        if !item_present {
            return None;
        }

        Some(V::from_bytes(value_data.into()))
    }

    /// Add or replace a value associated with `key`.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn insert(&mut self, key: &K, value: &V) -> Option<V> {
        // remove old data before insert new();
        let previous_value = self.remove(key);

        self.insert_data(&mut Key::new(key), value);

        previous_value
    }

    fn insert_data(&mut self, key: &mut Key<K>, value: &V) {
        let value_bytes = value.to_bytes();
        let chunks = value_bytes.chunks(V::CHUNK_SIZE as _);

        for chunk in chunks {
            let chunk = Chunk::new(chunk.to_vec());
            self.inner.insert(key.clone(), chunk);
            key.increase_chunk_index();
        }

        self.items_count += 1;
    }

    /// Remove a value associated with `key`.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let first_chunk_key = Key::new(key);
        let max_chunk_key = first_chunk_key.clone().with_max_chunk_index();
        let keys: Vec<Key<K>> = self
            .inner
            .range(first_chunk_key..=max_chunk_key)
            .map(|(k, _)| k)
            .collect();

        if keys.is_empty() {
            return None;
        }

        let mut value_bytes = Vec::new();
        for key in &keys {
            // We have got keys from the map, so they are present.
            // If something goes wrong, panic will help to avoid partly-removed items.
            let chunk = self.inner.remove(key).expect("the key present");
            value_bytes.extend_from_slice(chunk.data());
        }

        self.items_count -= 1;

        Some(V::from_bytes(value_bytes.into()))
    }

    /// Iterator for all stored key-value pairs.
    pub fn iter(&self) -> Iter<'_, M, K, V> {
        Iter(self.inner.iter().peekable())
    }

    /// Count of items in the map.
    pub fn len(&self) -> u64 {
        self.items_count
    }

    /// Is the map empty.
    pub fn is_empty(&self) -> bool {
        self.items_count == 0
    }

    /// Remove all entries from the map.
    pub fn clear(&mut self) {
        let keys: Vec<_> = self.inner.iter().map(|(k, _)| k).collect();
        for key in keys {
            self.inner.remove(&key);
        }
        self.items_count = 0;
    }
}

/// Provide information about the length of the value slice.
///
/// If value size is greater than `chunk_size()`, value will be split to several chunks,
/// and store each as particular entry in inner data structures.
///
/// More chunks count leads to more memory allocation operations.
/// But with big `chunk_size()` we lose space for small values,
/// because `chunk_size()` is a least allocation unit for any value.
pub trait SlicedStorable: Storable {
    const CHUNK_SIZE: ChunkSize;
}

/// Wrapper for the key.
///
/// # Memory layout
/// ```ignore
/// |-- size_prefix --|-- key_bytes --|-- chunk_index --|
/// ```
///
/// where:
/// - `size_prefix` is a len of `key_bytes`. Length of `size_prefix` depends on `K::max_size()`
/// and calculated in `Key::size_prefix_len()`.
/// - `key_bytes` is a result of the `<K as Storable>::to_bytes(key)` call. Length limited by the
/// `<K as BoundedStorable>::max_size()`.
/// - `chunk_index` is an index of chunk associated with a key instance. If inserted value split to `N`
/// chunks, then they stored as several entries. Each entry has unique key, with difference only in `chunk_index`.
/// In `get()` operation the value constructing from it's chunks. The `chunk_index` takes [`CHUNK_INDEX_LEN`] bytes.
struct Key<K: BoundedStorable> {
    data: Vec<u8>,
    _p: PhantomData<K>,
}

impl<K: BoundedStorable> Clone for Key<K> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            _p: PhantomData,
        }
    }
}

impl<K: BoundedStorable> PartialEq for Key<K> {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl<K: BoundedStorable> Eq for Key<K> {}

impl<K: BoundedStorable> PartialOrd for Key<K> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.data.partial_cmp(&other.data)
    }
}

impl<K: BoundedStorable> Ord for Key<K> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.data.cmp(&other.data)
    }
}

impl<K: BoundedStorable> Key<K> {
    /// Crate a new key.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn new(key: &K) -> Self {
        let key_bytes = key.to_bytes();
        assert!(key_bytes.len() <= K::MAX_SIZE as usize);

        let size_prefix_len = Self::size_prefix_len();
        let full_len = size_prefix_len + key_bytes.len() + CHUNK_INDEX_LEN;
        let mut data = Vec::with_capacity(full_len);
        data.extend_from_slice(&key_bytes.len().to_le_bytes()[..size_prefix_len]);
        data.extend_from_slice(&key_bytes);
        data.extend_from_slice(&[0u8; CHUNK_INDEX_LEN]);

        Self {
            data,
            _p: PhantomData,
        }
    }

    pub fn with_max_chunk_index(mut self) -> Self {
        let data_len = self.data.len();

        // last `CHUNK_INDEX_LEN` bytes is chunk index
        let chunk_index_bytes = &mut self.data[(data_len - CHUNK_INDEX_LEN)..];
        let chunk_index_arr = [u8::MAX; CHUNK_INDEX_LEN];

        chunk_index_bytes.copy_from_slice(&chunk_index_arr);
        self
    }

    pub fn increase_chunk_index(&mut self) {
        let data_len = self.data.len();

        // last `CHUNK_INDEX_LEN` bytes is chunk index
        let chunk_index_bytes = &mut self.data[(data_len - CHUNK_INDEX_LEN)..];

        let chunk_index_arr = chunk_index_bytes
            .try_into()
            .expect("the slice is always CHUNK_INDEX_LEN length");

        // store chunk index in big-endian format to preserve order of chunks in BTreeMap
        let chunk_index = ChunkIndex::from_be_bytes(chunk_index_arr);

        chunk_index_bytes.copy_from_slice(&(chunk_index + 1).to_be_bytes())
    }

    /// Prefix of key data, which is same for all chunks of the same value.
    pub fn prefix(&self) -> &[u8] {
        &self.data[..self.data.len() - CHUNK_INDEX_LEN]
    }

    /// Bytes of key `key: K`.
    /// Result of the `<K as Storable>::to_bytes(key)` call.
    pub fn key_data(&self) -> &[u8] {
        &self.data[Self::size_prefix_len()..self.data.len() - CHUNK_INDEX_LEN]
    }

    const fn size_prefix_len() -> usize {
        const U8_MAX: u32 = u8::MAX as u32;
        const U8_END: u32 = U8_MAX + 1;
        const U16_MAX: u32 = u16::MAX as u32;

        match K::MAX_SIZE {
            0..=U8_MAX => 1,
            U8_END..=U16_MAX => 2,
            _ => 4,
        }
    }
}

impl<K: BoundedStorable> Storable for Key<K> {
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        (&self.data).into()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self {
            data: bytes.to_vec(),
            _p: PhantomData,
        }
    }
}

impl<K: BoundedStorable> BoundedStorable for Key<K> {
    const MAX_SIZE: u32 = Self::size_prefix_len() as u32 + K::MAX_SIZE + CHUNK_INDEX_LEN as u32;
    const IS_FIXED_SIZE: bool = K::IS_FIXED_SIZE;
}

/// Wrapper for value chunks stored in inner [`StableBTreeMap`].
struct Chunk<V: SlicedStorable> {
    chunk: Vec<u8>,
    _p: PhantomData<V>,
}

impl<V: SlicedStorable> Chunk<V> {
    fn new(chunk: Vec<u8>) -> Self {
        Self {
            chunk,
            _p: PhantomData,
        }
    }

    fn data(&self) -> &[u8] {
        self.chunk.as_ref()
    }

    fn into_data(self) -> Vec<u8> {
        self.chunk
    }
}

impl<V: SlicedStorable> Storable for Chunk<V> {
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        (&self.chunk).into()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self {
            chunk: bytes.to_vec(),
            _p: PhantomData,
        }
    }
}

impl<V: SlicedStorable> BoundedStorable for Chunk<V> {
    const MAX_SIZE: u32 = V::CHUNK_SIZE as _;
    const IS_FIXED_SIZE: bool = false;
}

/// Iterator over values in unbounded map.
/// Constructs a value from chunks on each `next()` call.
pub struct Iter<'a, M, K, V>(Peekable<btreemap::Iter<'a, Key<K>, Chunk<V>, M>>)
where
    M: Memory + Clone,
    K: BoundedStorable,
    V: SlicedStorable;

impl<'a, M, K, V> Iterator for Iter<'a, M, K, V>
where
    M: Memory + Clone,
    K: BoundedStorable,
    V: SlicedStorable,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        let (key, chunk) = self.0.next()?;
        let mut value_data = chunk.into_data();

        while let Some((next_key, _)) = self.0.peek() {
            if next_key.prefix() != key.prefix() {
                break;
            }

            let new_chunk = self.0.next()?.1;
            value_data.extend_from_slice(new_chunk.data());
        }

        Some((
            K::from_bytes(key.key_data().into()),
            V::from_bytes(value_data.into()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use ic_exports::stable_structures::DefaultMemoryImpl;

    use super::StableUnboundedMap;
    use crate::test_utils;

    #[test]
    fn insert_get_test() {
        let mut map = StableUnboundedMap::new(DefaultMemoryImpl::default());
        assert!(map.is_empty());

        let long_str = test_utils::str_val(50000);
        let medium_str = test_utils::str_val(5000);
        let short_str = test_utils::str_val(50);

        map.insert(&0u32, &long_str);
        map.insert(&3u32, &medium_str);
        map.insert(&5u32, &short_str);

        assert_eq!(map.get(&0).as_ref(), Some(&long_str));
        assert_eq!(map.get(&3).as_ref(), Some(&medium_str));
        assert_eq!(map.get(&5).as_ref(), Some(&short_str));
    }

    #[test]
    fn insert_should_replace_previous_value() {
        let mut map = StableUnboundedMap::new(DefaultMemoryImpl::default());
        assert!(map.is_empty());

        let long_str = test_utils::str_val(50000);
        let short_str = test_utils::str_val(50);

        assert!(map.insert(&0u32, &long_str).is_none());
        let prev = map.insert(&0u32, &short_str).unwrap();

        assert_eq!(&prev, &long_str);
        assert_eq!(map.get(&0).as_ref(), Some(&short_str));
    }

    #[test]
    fn remove_test() {
        let mut map = StableUnboundedMap::new(DefaultMemoryImpl::default());

        let long_str = test_utils::str_val(50000);
        let medium_str = test_utils::str_val(5000);
        let short_str = test_utils::str_val(50);

        map.insert(&0u32, &long_str);
        map.insert(&3u32, &medium_str);
        map.insert(&5u32, &short_str);

        assert_eq!(map.remove(&3), Some(medium_str));

        assert_eq!(map.get(&0).as_ref(), Some(&long_str));
        assert_eq!(map.get(&5).as_ref(), Some(&short_str));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn iter_test() {
        let mut map = StableUnboundedMap::new(DefaultMemoryImpl::default());

        let strs = [
            test_utils::str_val(50),
            test_utils::str_val(5000),
            test_utils::str_val(50000),
        ];

        for i in 0..100u32 {
            map.insert(&i, &strs[i as usize % strs.len()]);
        }

        assert!(map.iter().all(|(k, v)| v == strs[k as usize % strs.len()]))
    }
}
