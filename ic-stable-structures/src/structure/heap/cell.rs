use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::Storable;

use crate::Result;

/// Stores value in heap memory, providing `get()/set()` API.
pub struct HeapCell<T: Storable>(T);

impl<T: Storable> HeapCell<T> {
    /// Create new storage for values with `T` type.
    pub fn new(_memory_id: MemoryId, value: T) -> Result<Self> {
        Ok(Self(value))
    }

    /// Returns reference to value stored in stable memory.
    pub fn get(&self) -> &T {
        &self.0
    }

    /// Updates value in stable memory.
    pub fn set(&mut self, value: T) -> Result<()> {
        self.0 = value;
        Ok(())
    }
}
