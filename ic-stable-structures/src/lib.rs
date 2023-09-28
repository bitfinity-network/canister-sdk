mod structure;

mod error;
#[cfg(feature = "memory-mapped-files")]
mod memory_mapped_files;
#[cfg(test)]
mod test_utils;

pub use error::{Error, Result};
pub use ic_exports::stable_structures::memory_manager::MemoryId;
pub use ic_exports::stable_structures::{BoundedStorable, Storable};

#[cfg(feature = "memory-mapped-files")]
pub use memory_mapped_files::MemoryMappedFileMemory;

pub use structure::*;
