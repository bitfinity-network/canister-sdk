use dfinity_stable_structures::memory_manager::{
    MemoryId, MemoryManager as IcMemoryManager, VirtualMemory,
};
use dfinity_stable_structures::Memory;

pub trait MemoryManager<M: Memory, T> {
    fn get(&self, id: T) -> M;
}

impl<M: Memory> MemoryManager<VirtualMemory<M>, u8> for IcMemoryManager<M> {
    fn get(&self, id: u8) -> VirtualMemory<M> {
        self.get(MemoryId::new(id.into()))
    }
}

impl<M: Memory> MemoryManager<VirtualMemory<M>, MemoryId> for IcMemoryManager<M> {
    fn get(&self, id: MemoryId) -> VirtualMemory<M> {
        self.get(id)
    }
}
