pub mod ring_buffer;

use dfinity_stable_structures::Storable;
pub use ring_buffer::{StableRingBuffer, StableRingBufferIndices};

pub type ChunkSize = u16;

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

pub struct Bounds {
    pub max_size: usize,
    pub is_fixed_size: bool,
    pub size_prefix_len: usize,
}

impl Bounds {
    pub const fn new(max_size: usize, is_fixed_size: bool) -> Self {
        Self {
            max_size,
            is_fixed_size,
            size_prefix_len: Bounds::size_prefix_len(max_size, is_fixed_size),
        }
    }

    pub const fn size_prefix_len(max_size: usize, is_fixed_size: bool) -> usize {
        if is_fixed_size {
            0
        } else if max_size <= u8::MAX as usize {
            1
        } else if max_size <= u16::MAX as usize {
            2
        } else {
            4
        }
    }
}
