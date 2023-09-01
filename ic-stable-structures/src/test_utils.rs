use std::borrow::Cow;

use ic_exports::stable_structures::{BoundedStorable, Storable};

use crate::structure::{ChunkSize, SlicedStorable};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringValue(pub String);

impl Storable for StringValue {
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        self.0.to_bytes()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self(String::from_bytes(bytes))
    }
}

impl SlicedStorable for StringValue {
    const CHUNK_SIZE: ChunkSize = 64;
}

pub fn str_val(len: usize) -> StringValue {
    let mut s = String::with_capacity(len);
    s.extend((0..len).map(|_| 'Q'));
    StringValue(s)
}

/// New type pattern used to implement `Storable` trait for all arrays.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Array<const N: usize>(pub [u8; N]);

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

impl<const N: usize> SlicedStorable for Array<N> {
    const CHUNK_SIZE: ChunkSize = 64;
}
