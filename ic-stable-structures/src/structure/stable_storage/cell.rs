use ic_exports::stable_structures::{cell, Memory, Storable};

use crate::structure::CellStructure;
use crate::Result;

/// Stores value in stable memory, providing `get()/set()` API.
pub struct StableCell<T: Storable, M: Memory>(cell::Cell<T, M>);

impl<T: Storable, M: Memory> StableCell<T, M> {
    /// Create new storage for values with `T` type.
    pub fn new(memory: M, value: T) -> Result<Self> {
        Ok(Self(cell::Cell::init(memory, value)?))
    }
}

impl<T: Storable, M: Memory> CellStructure<T> for StableCell<T, M> {
    fn get(&self) -> &T {
        self.0.get()
    }

    fn set(&mut self, value: T) -> Result<()> {
        self.0.set(value)?;
        Ok(())
    }
}
