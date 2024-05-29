pub mod ring_buffer;

pub use ring_buffer::{StableRingBuffer, StableRingBufferIndices};

/// A trait for types that have a minimum and maximum value.
pub trait Bounded<T> {
    const MIN: T;
    const MAX: T;
}

impl Bounded<u8> for u8 {
    const MIN: u8 = 0;
    const MAX: u8 = u8::MAX;
}

impl Bounded<u16> for u16 {
    const MIN: u16 = 0;
    const MAX: u16 = u16::MAX;
}

impl Bounded<u32> for u32 {
    const MIN: u32 = 0;
    const MAX: u32 = u32::MAX;
}

impl Bounded<u64> for u64 {
    const MIN: u64 = 0;
    const MAX: u64 = u64::MAX;
}

impl Bounded<u128> for u128 {
    const MIN: u128 = 0;
    const MAX: u128 = u128::MAX;
}

impl Bounded<usize> for usize {
    const MIN: usize = 0;
    const MAX: usize = usize::MAX;
}

impl Bounded<i8> for i8 {
    const MIN: i8 = i8::MIN;
    const MAX: i8 = i8::MAX;
}

impl Bounded<i16> for i16 {
    const MIN: i16 = i16::MIN;
    const MAX: i16 = i16::MAX;
}

impl Bounded<i32> for i32 {
    const MIN: i32 = i32::MIN;
    const MAX: i32 = i32::MAX;
}

impl Bounded<i64> for i64 {
    const MIN: i64 = i64::MIN;
    const MAX: i64 = i64::MAX;
}

impl Bounded<i128> for i128 {
    const MIN: i128 = i128::MIN;
    const MAX: i128 = i128::MAX;
}

impl Bounded<isize> for isize {
    const MIN: isize = isize::MIN;
    const MAX: isize = isize::MAX;
}

impl Bounded<f32> for f32 {
    const MIN: f32 = f32::MIN;
    const MAX: f32 = f32::MAX;
}

impl Bounded<f64> for f64 {
    const MIN: f64 = f64::MIN;
    const MAX: f64 = f64::MAX;
}

impl<const N: usize> Bounded<[u8; N]> for [u8; N] {
    const MIN: [u8; N] = [u8::MIN; N];
    const MAX: [u8; N] = [u8::MAX; N];
}
