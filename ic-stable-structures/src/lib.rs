mod structure;

mod error;
#[cfg(feature = "memory-mapped-files")]
mod memory_mapped_files;
mod memory_utils;
#[cfg(test)]
mod test_utils;

pub use dfinity_stable_structures as stable_structures;

pub use error::{Error, Result};
pub use stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
pub use stable_structures::{BoundedStorable, FileMemory, Storable, VectorMemory};

#[cfg(target_arch = "wasm32")]
pub use stable_structures::Ic0StableMemory;

#[cfg(feature = "memory-mapped-files")]
pub use memory_mapped_files::MemoryMappedFileMemory;

pub use memory_utils::{
    get_memory_by_id, DefaultMemoryManager, DefaultMemoryResourceType, DefaultMemoryType,
};

pub use structure::*;
