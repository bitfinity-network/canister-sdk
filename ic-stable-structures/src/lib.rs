mod structure;

mod error;
#[cfg(feature = "memory-mapped-files")]
mod memory_mapped_files;
#[cfg(test)]
mod test_utils;

pub use dfinity_stable_structures as stable_structures;

pub use error::{Error, Result};
pub use stable_structures::{BoundedStorable, Storable};

#[cfg(feature = "memory-mapped-files")]
pub use memory_mapped_files::MemoryMappedFileMemory;

pub use structure::*;
