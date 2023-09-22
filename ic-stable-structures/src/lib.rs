mod structure;

mod error;
#[cfg(feature = "memory-mapped-files")]
mod memory_mapped_files;
#[cfg(test)]
mod test_utils;

pub use error::{Error, Result};
pub use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::memory_manager::{self, VirtualMemory};
use ic_exports::stable_structures::DefaultMemoryImpl;
pub use ic_exports::stable_structures::{BoundedStorable, Storable};

pub type Memory = VirtualMemory<DefaultMemoryImpl>;

type MemoryManager = memory_manager::MemoryManager<DefaultMemoryImpl>;

pub use structure::*;
