use dfinity_stable_structures::{Memory, memory_manager::{MemoryId, MemoryManager as IcMemoryManager, VirtualMemory}};


pub trait MemoryManager<M: Memory, T> {
    fn get(&self, id: T) -> M;
}

impl <M: Memory, T: Into<u8>> MemoryManager<VirtualMemory<M>, T> for IcMemoryManager<M> {
    fn get(&self, id: T) -> VirtualMemory<M> {
        self.get(MemoryId::new(id.into()))
    }
}
