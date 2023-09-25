mod structure;

mod error;
#[cfg(feature = "memory-mapped-files")]
mod memory_mapped_files;
#[cfg(test)]
mod test_utils;

pub use error::{Error, Result};
pub use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::memory_manager::{self, VirtualMemory};
pub use ic_exports::stable_structures::{BoundedStorable, Storable};

#[cfg(feature = "memory-mapped-files")]
pub type DefaultMemoryType = std::rc::Rc<memory_mapped_files::GlobalMemoryMappedFileMemory>;
#[cfg(not(feature = "memory-mapped-files"))]
pub type DefaultMemoryType = ic_exports::stable_structures::DefaultMemoryImpl;

pub type Memory = VirtualMemory<DefaultMemoryType>;

type MemoryManager = memory_manager::MemoryManager<DefaultMemoryType>;

pub use structure::*;
