use dfinity_stable_structures::memory_manager::MemoryId;
use dfinity_stable_structures::Storable;

use crate::structure::CellStructure;
use crate::Result;

/// Stores value in heap memory, providing `get()/set()` API.
pub struct HeapCell<T: Storable>(T);

impl<T: Storable> HeapCell<T> {
    /// Create new storage for values with `T` type.
    pub fn new(_memory_id: MemoryId, value: T) -> Result<Self> {
        Ok(Self(value))
    }
}

impl<T: Storable> CellStructure<T> for HeapCell<T> {
    fn get(&self) -> &T {
        &self.0
    }

    fn set(&mut self, value: T) -> Result<()> {
        self.0 = value;
        Ok(())
    }
}
