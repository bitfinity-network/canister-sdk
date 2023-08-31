
use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::{cell, Storable};

use crate::{Memory, Result};
use super::get_memory_by_id;

/// Stores value in stable memory, providing `get()/set()` API.
pub struct StableCell<T: Storable>(cell::Cell<T, Memory>);

impl<T: Storable> StableCell<T> {
    /// Create new storage for values with `T` type.
    pub fn new(memory_id: MemoryId, value: T) -> Result<Self> {
        let memory = get_memory_by_id(memory_id);
        let cell = cell::Cell::init(memory, value)?;
        Ok(Self(cell))
    }

    /// Returns reference to value stored in stable memory.
    pub fn get(&self) -> &T {
        self.0.get()
    }

    /// Updates value in stable memory.
    pub fn set(&mut self, value: T) -> Result<()> {
        self.0.set(value)?;
        Ok(())
    }
}
