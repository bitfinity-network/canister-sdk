pub mod ring_buffer;

use candid::Principal;
pub use ring_buffer::{StableRingBuffer, StableRingBufferIndices};

/// A trait for types that have a minimum and maximum value.
pub trait Bounded {
    const MIN: Self;
    const MAX: Self;
}

impl Bounded for u8 {
    const MIN: u8 = 0;
    const MAX: u8 = u8::MAX;
}

impl Bounded for u16 {
    const MIN: u16 = 0;
    const MAX: u16 = u16::MAX;
}

impl Bounded for u32 {
    const MIN: u32 = 0;
    const MAX: u32 = u32::MAX;
}

impl Bounded for u64 {
    const MIN: u64 = 0;
    const MAX: u64 = u64::MAX;
}

impl Bounded for u128 {
    const MIN: u128 = 0;
    const MAX: u128 = u128::MAX;
}

impl Bounded for usize {
    const MIN: usize = 0;
    const MAX: usize = usize::MAX;
}

impl Bounded for i8 {
    const MIN: i8 = i8::MIN;
    const MAX: i8 = i8::MAX;
}

impl Bounded for i16 {
    const MIN: i16 = i16::MIN;
    const MAX: i16 = i16::MAX;
}

impl Bounded for i32 {
    const MIN: i32 = i32::MIN;
    const MAX: i32 = i32::MAX;
}

impl Bounded for i64 {
    const MIN: i64 = i64::MIN;
    const MAX: i64 = i64::MAX;
}

impl Bounded for i128 {
    const MIN: i128 = i128::MIN;
    const MAX: i128 = i128::MAX;
}

impl Bounded for isize {
    const MIN: isize = isize::MIN;
    const MAX: isize = isize::MAX;
}

impl Bounded for f32 {
    const MIN: f32 = f32::MIN;
    const MAX: f32 = f32::MAX;
}

impl Bounded for f64 {
    const MIN: f64 = f64::MIN;
    const MAX: f64 = f64::MAX;
}

impl<const N: usize> Bounded for [u8; N] {
    const MIN: [u8; N] = [u8::MIN; N];
    const MAX: [u8; N] = [u8::MAX; N];
}

/// Principal sorted by two fields:
/// 1) `len: u8` -> min = 0, max = 29;
/// 2) `bytes: [u8; Self::MAX_LENGTH_IN_BYTES]` -> min = [], max = [0xFF; 29];
impl Bounded for Principal {
    const MIN: Self = Principal::from_slice(&[]); // Management canister principal;
    const MAX: Self = Principal::from_slice(&[0xFF; 29]);
}

#[cfg(test)]
mod tests {
    use candid::Principal;

    use crate::Bounded;

    #[test]
    fn correct_principal_bounds() {
        let min_principal = Principal::MIN;
        let max_principal = Principal::MAX;

        let mut some_principals = vec![Principal::anonymous(), Principal::management_canister()];

        let other_principals_iter =
            (1..30).map(|i| Principal::from_slice(&vec![i as u8; i as usize]));
        some_principals.extend(other_principals_iter);

        for principal in some_principals {
            assert!(principal >= min_principal);
            assert!(principal <= max_principal);
        }
    }
}
