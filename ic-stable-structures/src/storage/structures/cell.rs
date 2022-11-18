use std::collections::{hash_map::Entry, HashMap};

use ic_exports::{
    ic_kit::ic,
    stable_structures::{cell, memory_manager::MemoryId, Storable},
    Principal,
};

use crate::{Memory, Result};

/// Stores value in stable memory, providing `get()/set()` API.
pub struct StableCell<T: Storable> {
    data: HashMap<Principal, cell::Cell<T, Memory>>,
    default_value: T,
    memory_id: MemoryId,
}

impl<T: Storable> StableCell<T> {
    /// Create new storage for values with `T` type.
    pub fn new(memory_id: MemoryId, value: T) -> Result<Self> {
        // Method returns Result to be compatible with wasm implementation.
        Ok(Self {
            data: HashMap::default(),
            default_value: value,
            memory_id,
        })
    }

    /// Returns reference to value stored in stable memory.
    pub fn get(&self) -> &T {
        let canister_id = ic::id();
        self.data
            .get(&canister_id)
            .map(|cell| cell.get())
            .unwrap_or(&self.default_value)
    }

    /// Updates value in stable memory.
    pub fn set(&mut self, value: T) -> Result<()> {
        let canister_id = ic::id();
        match self.data.entry(canister_id) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().set(value)?;
            }
            Entry::Vacant(entry) => {
                let memory = crate::get_memory_by_id(self.memory_id);
                entry.insert(cell::Cell::init(memory, value)?);
            }
        };
        Ok(())
    }
}
