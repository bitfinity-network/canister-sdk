use std::cell::RefCell;
use std::collections::HashMap;

use ic_exports::candid::Principal;
use ic_exports::ic_kit::ic;
use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::DefaultMemoryImpl;
pub use structures::{
    StableBTreeMap, StableCell, StableLog, StableMultimap, StableRingBuffer,
    StableRingBufferIndices, StableUnboundedMap, StableVec,
};

use crate::{Memory, MemoryManager};

pub mod structures;

#[derive(Default)]
struct Manager(HashMap<Principal, MemoryManager>);

impl Manager {
    // To reduce memory consumption in tests, use only 8KB preallocaions.
    const BUCKET_SIZE_IN_PAGES: u16 = 8;

    pub fn get(&mut self, memory_id: MemoryId) -> Memory {
        let canister_id = ic::id();
        self.0
            .entry(canister_id)
            .or_insert_with(|| {
                MemoryManager::init_with_bucket_size(
                    DefaultMemoryImpl::default(),
                    Self::BUCKET_SIZE_IN_PAGES,
                )
            })
            .get(memory_id)
    }
}

thread_local! {
    // The memory manager is used for simulating multiple memories. Given a `MemoryId` it can
    // return a memory that can be used by stable structures.
    static MANAGER: RefCell<Manager> = RefCell::default();
}

// Return memory by `MemoryId`.
// Each instance of stable structures must have unique `MemoryId`;
pub fn get_memory_by_id(id: MemoryId) -> Memory {
    MANAGER.with(|mng| mng.borrow_mut().get(id))
}
