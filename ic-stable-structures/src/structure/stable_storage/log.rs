use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::{log, Storable};

use crate::structure::LogStructure;
use crate::{Error, Memory, Result};

/// Stores list of immutable values in stable memory.
/// Provides only `append()` and `get()` operations.
pub struct StableLog<T: Storable>(Option<log::Log<T, Memory, Memory>>);

impl<T: Storable> StableLog<T> {
    /// Create new storage for values with `T` type.
    pub fn new(index_memory_id: MemoryId, data_memory_id: MemoryId) -> Result<Self> {
        // Method returns Result to be compatible with wasm implementation.

        // Index and data should be stored in different memories.
        assert_ne!(index_memory_id, data_memory_id);

        let index_memory = crate::get_memory_by_id(index_memory_id);
        let data_memory = crate::get_memory_by_id(data_memory_id);

        let inner = log::Log::init(index_memory, data_memory)?;
        Ok(Self(Some(inner)))
    }

    fn get_inner(&self) -> &log::Log<T, Memory, Memory> {
        self.0.as_ref().expect("inner log is always present")
    }

    fn mut_inner(&mut self) -> &mut log::Log<T, Memory, Memory> {
        self.0.as_mut().expect("inner log is always present")
    }
}

impl<T: Storable> LogStructure<T> for StableLog<T> {
    fn get(&self, index: u64) -> Option<T> {
        self.get_inner().get(index)
    }

    fn append(&mut self, value: T) -> Result<u64> {
        self.mut_inner()
            .append(&value)
            .map_err(|_| Error::OutOfStableMemory)
    }

    fn len(&self) -> u64 {
        self.get_inner().len()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn clear(&mut self) {
        let inner = self.0.take().expect("inner log is always present");
        let (index_mem, data_mem) = inner.into_memories();
        self.0 = Some(log::Log::new(index_mem, data_mem));
    }
}
