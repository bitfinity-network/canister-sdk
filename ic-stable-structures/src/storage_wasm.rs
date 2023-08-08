use std::cell::RefCell;

use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::DefaultMemoryImpl;
pub use structures::{
    StableBTreeMap, StableCell, StableLog, StableMultimap, StableRingBuffer,
    StableRingBufferIndices, StableUnboundedMap, StableVec,
};

use crate::{Memory, MemoryManager};

#[path = "storage_wasm/structures_wasm.rs"]
mod structures;

thread_local! {
    // The memory manager is used for simulating multiple memories. Given a `MemoryId` it can
    // return a memory that can be used by stable structures.
    static MANAGER: RefCell<MemoryManager> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
}

// Return memory by `MemoryId`.
// Each instance of stable structures must have unique `MemoryId`;
pub fn get_memory_by_id(id: MemoryId) -> Memory {
    MANAGER.with(|mng| mng.borrow_mut().get(id))
}
