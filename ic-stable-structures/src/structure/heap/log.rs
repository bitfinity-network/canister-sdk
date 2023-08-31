use ic_exports::stable_structures::{Storable, memory_manager::MemoryId};

use crate::Result;


/// Stores list of immutable values in heap memory.
/// Provides only `append()` and `get()` operations.
pub struct HeapLog<T: Storable + Clone>(Vec<T>);

impl<T: Storable + Clone> HeapLog<T> {

    /// Create new storage for values with `T` type.
    pub fn new(_index_memory_id: MemoryId, _data_memory_id: MemoryId) -> Result<Self> {
        Ok(Self(vec![]))
    }

    /// Returns reference to value stored in stable memory.
    pub fn get(&self, index: u64) -> Option<T> {
        self.0.get(index as usize).cloned()
    }

    /// Updates value in stable memory.
    pub fn append(&mut self, value: T) -> Result<u64> {
        self.0.push(value);
        Ok(self.len())
    }

    /// Number of values in the log.
    pub fn len(&self) -> u64 {
        self.0.len() as u64
    }

    // Returns true, if the Log doesn't contain any values.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Remove all items from the log.
    pub fn clear(&mut self) {
        self.0.clear()
    }

}

