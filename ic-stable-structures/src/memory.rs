use dfinity_stable_structures::{Memory, memory_manager::{MemoryId, MemoryManager as IcMemoryManager, VirtualMemory}};


pub trait MemoryManager<M: Memory> {
    fn get(&self, id: MemoryId) -> M;
}

impl <M: Memory> MemoryManager<VirtualMemory<M>> for IcMemoryManager<M> {
    fn get(&self, id: MemoryId) -> VirtualMemory<M> {
        self.get(id)
    }
}