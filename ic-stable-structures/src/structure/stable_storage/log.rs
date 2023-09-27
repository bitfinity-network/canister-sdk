use ic_exports::stable_structures::{log, Storable, Memory};

use crate::structure::LogStructure;
use crate::{Error, Result};

/// Stores list of immutable values in stable memory.
/// Provides only `append()` and `get()` operations.
pub struct StableLog<T: Storable, M: Memory>(Option<log::Log<T, M, M>>);

impl<T: Storable, M: Memory> StableLog<T, M> {
    /// Create new storage for values with `T` type.
    pub fn new(index_memory: M, data_memory: M) -> Result<Self> {
        // Method returns Result to be compatible with wasm implementation.
        Ok(Self(Some(log::Log::init(index_memory, data_memory)?)))
    }

    fn get_inner(&self) -> &log::Log<T, M, M> {
        self.0.as_ref().expect("inner log is always present")
    }

    fn mut_inner(&mut self) -> &mut log::Log<T, M, M> {
        self.0.as_mut().expect("inner log is always present")
    }
}

impl<T: Storable, M: Memory> LogStructure<T> for StableLog<T, M> {
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
        let (index_mem, data_mem) = self.0.take().expect("inner log is laways present").into_memories();
        self.0 = Some(log::Log::new(index_mem, data_mem));
    }
}
