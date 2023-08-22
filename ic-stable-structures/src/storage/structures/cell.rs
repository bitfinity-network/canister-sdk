use std::collections::hash_map::Entry;
use std::collections::HashMap;

use ic_exports::candid::Principal;
use ic_exports::ic_kit::ic;
use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::{cell, Storable};

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

#[cfg(test)]
mod tests {
    use ic_exports::ic_kit::inject::get_context;
    use ic_exports::ic_kit::{mock_principals, MockContext};
    use ic_exports::stable_structures::memory_manager::MemoryId;

    use super::StableCell;

    #[test]
    fn cell_works() {
        MockContext::new().inject();
        let mut cell = StableCell::new(MemoryId::new(0), 42u32).unwrap();
        assert_eq!(*cell.get(), 42);
        cell.set(100).unwrap();
        assert_eq!(*cell.get(), 100);
    }

    #[test]
    fn two_canisters() {
        let mut cell = StableCell::new(MemoryId::new(0), 42u32).unwrap();

        MockContext::new()
            .with_id(mock_principals::alice())
            .inject();
        cell.set(42).unwrap();

        get_context().update_id(mock_principals::bob());
        cell.set(100).unwrap();

        get_context().update_id(mock_principals::alice());
        assert_eq!(*cell.get(), 42);

        get_context().update_id(mock_principals::bob());
        assert_eq!(*cell.get(), 100);
    }
}
