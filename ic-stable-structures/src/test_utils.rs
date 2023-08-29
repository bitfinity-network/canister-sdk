use std::borrow::Cow;

use ic_exports::stable_structures::Storable;

use crate::structure::common::unbounded::SlicedStorable;
use crate::ChunkSize;

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
