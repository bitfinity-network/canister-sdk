pub mod ring_buffer;

use ic_exports::stable_structures::Storable;
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
