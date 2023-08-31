pub mod structure;

mod error;
#[cfg(test)]
mod test_utils;

pub use error::{Error, Result};
pub use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::memory_manager::{self, VirtualMemory};
use ic_exports::stable_structures::DefaultMemoryImpl;
pub use ic_exports::stable_structures::{BoundedStorable, Storable};
pub use structure::common::ring_buffer::{Indices as StableRingBufferIndices, StableRingBuffer};
pub use structure::common::unbounded::{ChunkSize, Iter as UnboundedIter, SlicedStorable};

pub type Memory = VirtualMemory<DefaultMemoryImpl>;

type MemoryManager = memory_manager::MemoryManager<DefaultMemoryImpl>;

// #[cfg(not(feature = "default-heap-structures"))]
pub use structure::stable_storage::*;

pub use structure::cache::btreemap::CachedStableBTreeMap;

pub use structure::heap::*;

// #[cfg(feature = "default-heap-structures")]
// pub use structure::heap::{
//     StableBTreeMap, StableCell, StableLog, StableMultimap, StableUnboundedMap,
//     StableVec,
// };