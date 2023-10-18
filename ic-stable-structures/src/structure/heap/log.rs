use std::marker::PhantomData;

use dfinity_stable_structures::Storable;

use crate::structure::LogStructure;
use crate::Result;

/// Stores list of immutable values in heap memory.
/// Provides only `append()` and `get()` operations.
pub struct HeapLog<T: Storable + Clone, M>(Vec<T>, PhantomData<M>);

impl<T: Storable + Clone, M> HeapLog<T, M> {
    /// Create new storage for values with `T` type.
    pub fn new(_index_memory: M, _data_memory: M) -> Result<Self> {
        Ok(Self(vec![], Default::default()))
    }
}

impl<T: Storable + Clone, M> LogStructure<T> for HeapLog<T, M> {
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
