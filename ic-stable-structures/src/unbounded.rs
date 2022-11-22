use std::{marker::PhantomData, mem};

use ic_exports::stable_structures::{BoundedStorable, Memory, StableBTreeMap, Storable};

type ChunkIndex = u16;
const CHUNK_INDEX_LEN: usize = mem::size_of::<ChunkIndex>();

type ChunkSize = u16;
const CHUNK_SIZE_LEN: usize = mem::size_of::<ChunkSize>();

pub struct StableUnboundedMap<M, K, V>
where
    M: Memory + Clone,
    K: BoundedStorable,
    V: SlicedStorable,
{
    inner: StableBTreeMap<M, Key<K>, Value<V>>,
    _p: PhantomData<V>,
}

pub trait SlicedStorable: Storable {
    fn chunk_size() -> ChunkSize;
}

struct Key<K: BoundedStorable> {
    key: K,
    chunk_index: ChunkIndex,
}

impl<K: BoundedStorable> Storable for Key<K> {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let mut data = self.key.to_bytes().to_vec();
        let new_len = data.len() + CHUNK_INDEX_LEN;
        data.resize(new_len, 0);
        data.extend_from_slice(&self.chunk_index.to_be_bytes());
        data.into()
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        let key_end = bytes.len() - CHUNK_INDEX_LEN;
        let key = K::from_bytes(bytes[..key_end].to_vec());

        let buf = <[u8; 2]>::try_from(&bytes[key_end..]).expect("slice has CHUNK_INDEX_LEN bytes");
        let chunk_index = ChunkIndex::from_be_bytes(buf);
        Self { key, chunk_index }
    }
}

impl<K: BoundedStorable> BoundedStorable for Key<K> {
    fn max_size() -> u32 {
        K::max_size() + CHUNK_INDEX_LEN as u32
    }
}

struct Value<V: SlicedStorable> {
    chunk: Vec<u8>,
    _p: PhantomData<V>,
}

impl<V: SlicedStorable> Storable for Value<V> {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        (&self.chunk).into()
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            chunk: bytes,
            _p: PhantomData::default(),
        }
    }
}

impl<V: SlicedStorable> BoundedStorable for Value<V> {
    fn max_size() -> u32 {
        V::chunk_size() as u32 + CHUNK_SIZE_LEN as u32
    }
}
