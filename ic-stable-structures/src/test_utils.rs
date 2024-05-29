use std::borrow::Cow;

use dfinity_stable_structures::storable::Bound;
use dfinity_stable_structures::Storable;

use crate::Bounded;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringValue(pub String);

impl Storable for StringValue {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        self.0.to_bytes()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self(String::from_bytes(bytes))
    }
}

pub fn str_val(len: usize) -> StringValue {
    let mut s = String::with_capacity(len);
    s.extend((0..len).map(|_| 'Q'));
    StringValue(s)
}

/// New type pattern used to implement `Storable` trait for all arrays.
#[derive(Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct Array<const N: usize>(pub [u8; N]);

/// Implement `Bounded` for all arrays.
impl<const N: usize> Bounded<Array<N>> for Array<N> {
    const MIN: Array<N> = Array([0; N]);
    const MAX: Array<N> = Array([u8::MAX; N]);
}

impl<const N: usize> Storable for Array<N> {
    const BOUND: Bound = Bound::Bounded {
        max_size: N as u32,
        is_fixed_size: true,
    };

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Owned(self.0.to_vec())
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        let mut buf = [0u8; N];
        buf.copy_from_slice(&bytes);
        Array(buf)
    }
}
