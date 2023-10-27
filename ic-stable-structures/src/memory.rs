use dfinity_stable_structures::memory_manager::{
    MemoryId, MemoryManager as IcMemoryManager, VirtualMemory,
};
use dfinity_stable_structures::{DefaultMemoryImpl, Memory};

/// A memory manager that can return multiple memories.
pub trait MemoryManager<M: Memory, T> {

    /// Return a new memory based on a unique ID
    fn get(&self, id: T) -> M;
}

impl<M: Memory> MemoryManager<VirtualMemory<M>, u8> for IcMemoryManager<M> {
    fn get(&self, id: u8) -> VirtualMemory<M> {
        self.get(MemoryId::new(id))
    }
}

impl<M: Memory> MemoryManager<VirtualMemory<M>, MemoryId> for IcMemoryManager<M> {
    fn get(&self, id: MemoryId) -> VirtualMemory<M> {
        self.get(id)
    }
}

/// Returns a VirtualMemory MemoryManager that uses the default IC memory
pub fn default_ic_memory_manager() -> IcMemoryManager<DefaultMemoryImpl> {
    IcMemoryManager::init(DefaultMemoryImpl::default())
}
