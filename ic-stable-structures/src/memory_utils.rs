use std::thread::LocalKey;

use dfinity_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    Memory,
};

#[cfg(not(target_arch = "wasm32"))]
pub type DefaultMemoryResourceType = dfinity_stable_structures::VectorMemory;

#[cfg(target_arch = "wasm32")]
pub type DefaultMemoryResourceType = dfinity_stable_structures::Ic0StableMemory;

pub type DefaultMemoryType = VirtualMemory<DefaultMemoryResourceType>;
pub type DefaultMemoryManager = MemoryManager<DefaultMemoryResourceType>;

/// Returns virtual memory by id
pub fn get_memory_by_id<M: Memory>(
    memory_manager: &'static LocalKey<MemoryManager<M>>,
    id: MemoryId,
) -> VirtualMemory<M> {
    memory_manager.with(|memory_manager| memory_manager.get(id))
}
