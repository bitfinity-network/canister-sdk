mod multimap;
mod unbounded;

#[cfg(not(target_arch = "wasm32"))]
mod storage;

#[cfg(target_arch = "wasm32")]
#[path = "storage_wasm.rs"]
mod storage;

mod error;
#[cfg(test)]
mod test_utils;

pub use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::memory_manager::{self, VirtualMemory};
use ic_exports::stable_structures::DefaultMemoryImpl;
pub use ic_exports::stable_structures::Storable;

pub use error::{Error, Result};
pub use multimap::{Iter, RangeIter};
pub use storage::{
    get_memory_by_id, StableBTreeMap, StableCell, StableLog, StableMultimap, StableUnboundedMap,
};
pub use unbounded::{ChunkSize, Iter as UnboundedIter};

pub type Memory = VirtualMemory<DefaultMemoryImpl>;

type MemoryManager = memory_manager::MemoryManager<DefaultMemoryImpl>;
