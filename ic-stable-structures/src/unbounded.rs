use crate::{Error, Result};

use std::{iter::Peekable, marker::PhantomData, mem};

use ic_exports::stable_structures::{btreemap, BoundedStorable, Memory, StableBTreeMap, Storable};

pub type ChunkSize = u16;

type ChunkIndex = u16;

const CHUNK_INDEX_LEN: usize = mem::size_of::<ChunkIndex>();

pub struct StableUnboundedMap<M, K, V>
where
    M: Memory + Clone,
    K: BoundedStorable,
    V: SlicedStorable,
{
    inner: StableBTreeMap<M, Key<K>, Chunk<V>>,
    items_count: u64,
}

impl<M, K, V> StableUnboundedMap<M, K, V>
where
    M: Memory + Clone,
    K: BoundedStorable,
    V: SlicedStorable,
{
    pub fn new(memory: M) -> Self {
        Self {
            inner: StableBTreeMap::init(memory),
            items_count: 0,
        }
    }

    /// Return value associated with `key`.
    pub fn get(&self, key: &K) -> Option<V> {
        let key_prefix = Key::create_prefix(key).ok()?;
        let mut value_data = Vec::new();
        for (_, chunk) in self.inner.range(key_prefix, None) {
            value_data.extend_from_slice(&chunk.to_bytes())
        }

        Some(V::from_bytes(value_data))
    }

    /// Add or replace value associated with `key`.
    pub fn insert(&mut self, key: &K, value: &V) -> Result<()> {
        let mut inner_key = Key::new(key)?;

        let insert_result = self.insert_data(&mut inner_key, value);

        if insert_result.is_err() && inner_key.get_chunk_index() > 0 {
            // if insert failed and at least one chunk inserted, then remove the inserted chunks.
            self.remove(key);
        }

        insert_result
    }

    fn insert_data(&mut self, key: &mut Key<K>, value: &V) -> Result<()> {
        let value_bytes = value.to_bytes();
        let chunks = value_bytes.chunks(V::chunk_size() as _);

        for chunk in chunks {
            let chunk = Chunk::new(chunk.to_vec());
            self.inner.insert(key.clone(), chunk)?;
            key.increase_chunk_index();
        }

        self.items_count += 1;

        Ok(())
    }

    /// Remove value associated with `key`.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let key_prefix = Key::create_prefix(key).ok()?;
        let keys: Vec<Key<K>> = self.inner.range(key_prefix, None).map(|(k, _)| k).collect();

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

        Some(V::from_bytes(value_bytes))
    }

    /// List all currently stored key-value pairs.
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
}

pub trait SlicedStorable: Storable {
    fn chunk_size() -> ChunkSize;
}

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

impl<K: BoundedStorable> Key<K> {
    pub fn new(key: &K) -> Result<Self> {
        let key_bytes = key.to_bytes();
        if key_bytes.len() > K::max_size() as usize {
            return Err(Error::ValueTooLarge(key_bytes.len() as _));
        }

        let size_prefix_len = Self::size_prefix_len();
        let full_len = size_prefix_len + key_bytes.len() + CHUNK_INDEX_LEN;
        let mut data = Vec::with_capacity(full_len);
        data.extend_from_slice(&key_bytes.len().to_le_bytes()[..size_prefix_len]);
        data.extend_from_slice(&key_bytes);
        data.extend_from_slice(&[0u8; CHUNK_INDEX_LEN]);

        Ok(Self {
            data,
            _p: PhantomData,
        })
    }

    pub fn increase_chunk_index(&mut self) {
        let data_len = self.data.len();

        // last `CHUNK_INDEX_LEN` bytes is chunk index
        let chunk_index_bytes = &mut self.data[(data_len - CHUNK_INDEX_LEN as usize)..];

        let chunk_index_arr = chunk_index_bytes
            .try_into()
            .expect("CHUNK_INDEX_LEN bytes in slice");

        // store chunk index in big-endian format to preserve order of chunks in BTreeMap
        let chunk_index = ChunkIndex::from_be_bytes(chunk_index_arr);

        chunk_index_bytes.copy_from_slice(&(chunk_index + 1).to_be_bytes())
    }

    pub fn get_chunk_index(&self) -> ChunkIndex {
        let data_len = self.data.len();

        // last `CHUNK_INDEX_LEN` bytes is chunk index
        let chunk_index_bytes = &self.data[(data_len - CHUNK_INDEX_LEN as usize)..];

        let chunk_index_arr = chunk_index_bytes
            .try_into()
            .expect("CHUNK_INDEX_LEN bytes in slice");

        ChunkIndex::from_be_bytes(chunk_index_arr)
    }

    pub fn prefix(&self) -> &[u8] {
        &self.data[..self.data.len() - CHUNK_INDEX_LEN]
    }

    pub fn key_data(&self) -> &[u8] {
        &self.data[Self::size_prefix_len()..self.data.len() - CHUNK_INDEX_LEN]
    }

    pub fn create_prefix(key: &K) -> Result<Vec<u8>> {
        let key_bytes = key.to_bytes();
        if key_bytes.len() > K::max_size() as usize {
            return Err(Error::ValueTooLarge(key_bytes.len() as _));
        }

        let size_prefix_len = Self::size_prefix_len();
        let full_len = size_prefix_len + key_bytes.len();
        let mut data = Vec::with_capacity(full_len);
        data.extend_from_slice(&key_bytes.len().to_le_bytes()[..size_prefix_len]);
        data.extend_from_slice(&key_bytes);

        Ok(data)
    }

    fn size_prefix_len() -> usize {
        const U8_MAX: u32 = u8::MAX as u32;
        const U8_END: u32 = U8_MAX + 1;
        const U16_MAX: u32 = u16::MAX as u32;

        match K::max_size() {
            0..=U8_MAX => 1,
            U8_END..=U16_MAX => 2,
            _ => 4,
        }
    }
}

impl<K: BoundedStorable> Storable for Key<K> {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        (&self.data).into()
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            data: bytes,
            _p: PhantomData,
        }
    }
}

impl<K: BoundedStorable> BoundedStorable for Key<K> {
    fn max_size() -> u32 {
        Self::size_prefix_len() as u32 + K::max_size() + CHUNK_INDEX_LEN as u32
    }
}

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
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        (&self.chunk).into()
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            chunk: bytes,
            _p: PhantomData,
        }
    }
}

impl<V: SlicedStorable> BoundedStorable for Chunk<V> {
    fn max_size() -> u32 {
        V::chunk_size() as u32
    }
}

pub struct Iter<'a, M, K, V>(Peekable<btreemap::Iter<'a, M, Key<K>, Chunk<V>>>)
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

        loop {
            let next_key = match self.0.peek() {
                Some((k, _)) => k,
                None => break,
            };

            if next_key.prefix() != key.prefix() {
                break;
            }

            let new_chunk = self.0.next()?.1;
            value_data.extend_from_slice(new_chunk.data());
        }

        Some((
            K::from_bytes(key.key_data().to_vec()),
            V::from_bytes(value_data),
        ))
    }
}

#[cfg(test)]
mod tests {
    use ic_exports::stable_structures::DefaultMemoryImpl;

    use crate::test_utils;

    use super::StableUnboundedMap;

    #[test]
    fn insert_get_test() {
        let mut map = StableUnboundedMap::new(DefaultMemoryImpl::default());
        assert!(map.is_empty());

        let long_str = test_utils::str_val(50000);
        let medium_str = test_utils::str_val(5000);
        let short_str = test_utils::str_val(50);

        map.insert(&0u32, &long_str).unwrap();
        map.insert(&3u32, &medium_str).unwrap();
        map.insert(&5u32, &short_str).unwrap();
        assert_eq!(map.get(&0).as_ref(), Some(&long_str));
        assert_eq!(map.get(&3).as_ref(), Some(&medium_str));
        assert_eq!(map.get(&5).as_ref(), Some(&short_str));
    }

    #[test]
    fn remove_test() {
        let mut map = StableUnboundedMap::new(DefaultMemoryImpl::default());

        let long_str = test_utils::str_val(50000);
        let medium_str = test_utils::str_val(5000);
        let short_str = test_utils::str_val(50);

        map.insert(&0u32, &long_str).unwrap();
        map.insert(&3u32, &medium_str).unwrap();
        map.insert(&5u32, &short_str).unwrap();

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
            map.insert(&i, &strs[i as usize % strs.len()]).unwrap();
        }

        assert!(map.iter().all(|(k, v)| v == strs[k as usize % strs.len()]))
    }
}
