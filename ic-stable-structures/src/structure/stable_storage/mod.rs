use std::cell::RefCell;

use crate::{Memory, MemoryManager, DefaultMemoryType};
use ic_exports::stable_structures::memory_manager::MemoryId;

mod btreemap;
mod cell;
mod log;
mod multimap;
mod unbounded;
mod vec;

pub use btreemap::StableBTreeMap;
pub use cell::StableCell;
pub use log::StableLog;
pub use multimap::{StableMultimap, StableMultimapIter, StableMultimapRangeIter};
pub use unbounded::{StableUnboundedIter, StableUnboundedMap};
pub use vec::StableVec;

thread_local! {
    // The memory manager is used for simulating multiple memories. Given a `MemoryId` it can
    // return a memory that can be used by stable structures.
    static MANAGER: RefCell<MemoryManager> =
        RefCell::new(MemoryManager::init(DefaultMemoryType::default()));
}

// Return memory by `MemoryId`.
// Each instance of stable structures must have unique `MemoryId`;
pub fn get_memory_by_id(id: MemoryId) -> Memory {
    MANAGER.with(|mng| mng.borrow_mut().get(id))
}
