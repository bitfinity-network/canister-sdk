use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::{cell, Storable};

use super::get_memory_by_id;
use crate::structure::CellStructure;
use crate::{Memory, Result};

/// Stores value in stable memory, providing `get()/set()` API.
pub struct StableCell<T: Storable>(cell::Cell<T, Memory>);

impl<T: Storable> StableCell<T> {
    /// Create new storage for values with `T` type.
    pub fn new(memory_id: MemoryId, value: T) -> Result<Self> {
        let memory = get_memory_by_id(memory_id);
        let cell = cell::Cell::init(memory, value)?;
        Ok(Self(cell))
    }
}

impl<T: Storable> CellStructure<T> for StableCell<T> {
    fn get(&self) -> &T {
        self.0.get()
    }

    fn set(&mut self, value: T) -> Result<()> {
        self.0.set(value)?;
        Ok(())
    }
}
