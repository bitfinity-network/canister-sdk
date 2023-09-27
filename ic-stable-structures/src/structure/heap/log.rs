use dfinity_stable_structures::{memory_manager::MemoryId, Storable};

use crate::{structure::LogStructure, Result};

/// Stores list of immutable values in heap memory.
/// Provides only `append()` and `get()` operations.
pub struct HeapLog<T: Storable + Clone>(Vec<T>);

impl<T: Storable + Clone> HeapLog<T> {
    /// Create new storage for values with `T` type.
    pub fn new(_index_memory_id: MemoryId, _data_memory_id: MemoryId) -> Result<Self> {
        Ok(Self(vec![]))
    }
}

impl<T: Storable + Clone> LogStructure<T> for HeapLog<T> {
    fn get(&self, index: u64) -> Option<T> {
        self.0.get(index as usize).cloned()
    }

    fn append(&mut self, value: T) -> Result<u64> {
        self.0.push(value);
        Ok(self.len())
    }

    fn len(&self) -> u64 {
        self.0.len() as u64
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn clear(&mut self) {
        self.0.clear()
    }
}
