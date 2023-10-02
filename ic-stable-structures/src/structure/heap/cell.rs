use std::marker::PhantomData;

use dfinity_stable_structures::Storable;

use crate::structure::CellStructure;
use crate::Result;

/// Stores value in heap memory, providing `get()/set()` API.
pub struct HeapCell<T: Storable, M>(T, PhantomData<M>);

impl<T: Storable, M> HeapCell<T, M> {
    /// Create new storage for values with `T` type.
    pub fn new(_memory: M, value: T) -> Result<Self> {
        Ok(Self(value, Default::default()))
    }
}

impl<T: Storable, M> CellStructure<T> for HeapCell<T, M> {
    fn get(&self) -> &T {
        &self.0
    }

    fn set(&mut self, value: T) -> Result<()> {
        self.0 = value;
        Ok(())
    }
}
