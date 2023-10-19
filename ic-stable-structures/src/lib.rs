mod structure;

mod error;
#[cfg(feature = "memory-mapped-files-memory")]
mod memory_mapped_files;

#[cfg(test)]
mod test_utils;

pub use dfinity_stable_structures as stable_structures;
pub use error::{Error, Result};

#[cfg(feature = "memory-mapped-files-memory")]
pub use memory_mapped_files::MemoryMappedFileMemory;

pub use stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
pub use stable_structures::storable::Bound;
pub use stable_structures::{FileMemory, Storable, VectorMemory};
pub use structure::*;
