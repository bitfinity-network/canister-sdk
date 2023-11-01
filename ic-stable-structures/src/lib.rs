mod structure;

mod error;
mod memory;
#[cfg(feature = "memory-mapped-files-memory")]
mod memory_mapped_files;

#[cfg(test)]
mod test_utils;

pub use dfinity_stable_structures as stable_structures;
pub use error::{Error, Result};
pub use memory::*;
#[cfg(feature = "memory-mapped-files-memory")]
pub use memory_mapped_files::*;
pub use stable_structures::memory_manager::{
    MemoryId, MemoryManager as IcMemoryManager, VirtualMemory,
};
pub use stable_structures::storable::Bound;
pub use stable_structures::{FileMemory, Storable, VectorMemory};
pub use structure::*;
