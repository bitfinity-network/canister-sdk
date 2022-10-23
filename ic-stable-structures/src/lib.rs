mod multimap;

#[cfg(not(target_arch = "wasm32"))]
mod storage;

#[cfg(target_arch = "wasm32")]
#[path = "storage_wasm.rs"]
mod storage;

mod error;

pub use error::Error;
pub use ic_exports::stable_structures::{memory_manager::MemoryId, Storable};
use ic_exports::stable_structures::{
    memory_manager::{self, VirtualMemory},
    DefaultMemoryImpl,
};
pub use multimap::{Iter, RangeIter};
pub use storage::{get_memory_by_id, StableBTreeMap, StableCell, StableMultimap};

pub type Memory = VirtualMemory<DefaultMemoryImpl>;

type MemoryManager = memory_manager::MemoryManager<DefaultMemoryImpl>;
