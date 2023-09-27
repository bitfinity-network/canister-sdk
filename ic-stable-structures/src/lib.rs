mod structure;

mod error;
#[cfg(test)]
mod test_utils;

pub use dfinity_stable_structures as stable_structures;

pub use error::{Error, Result};
pub use stable_structures::memory_manager::MemoryId;
use stable_structures::memory_manager::{self, VirtualMemory};
use stable_structures::DefaultMemoryImpl;
pub use stable_structures::{BoundedStorable, Storable};

pub type Memory = VirtualMemory<DefaultMemoryImpl>;

type MemoryManager = memory_manager::MemoryManager<DefaultMemoryImpl>;

pub use structure::*;
