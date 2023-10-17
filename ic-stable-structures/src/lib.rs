mod structure;

mod error;
#[cfg(feature = "memory-mapped-files-memory")]
mod memory_mapped_files;
mod memory_utils;
#[cfg(test)]
mod test_utils;

pub use dfinity_stable_structures as stable_structures;

pub use error::{Error, Result};
pub use stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
pub use stable_structures::{FileMemory, Storable, VectorMemory};

#[cfg(target_family = "wasm")]
pub use stable_structures::Ic0StableMemory;

#[cfg(feature = "memory-mapped-files-memory")]
pub use memory_mapped_files::MemoryMappedFileMemory;

pub use memory_utils::{
    get_memory_by_id, DefaultMemoryManager, DefaultMemoryResourceType, DefaultMemoryType,
};

pub use structure::*;

// pub mod fff {

//     pub trait Storable {
//         /// The size bounds of the type.
//         const BOUND: Bound;
//     }

//     pub enum Bound {
//         Unbounded,

//         Bounded {
//             max_size: u32,

//             is_fixed_size: bool,
//         },
//     }

//     impl Bound {
//         pub const fn max_size(&self) -> u32 {
//             if let Bound::Bounded { max_size, .. } = self {
//                 *max_size
//             } else {
//                 panic!("Cannot get max size of unbounded type.");
//             }
//         }

//         /// Returns true if the type is fixed in size, false otherwise.
//         pub const fn is_fixed_size(&self) -> bool {
//             if let Bound::Bounded { is_fixed_size, .. } = self {
//                 *is_fixed_size
//             } else {
//                 false
//             }
//         }
//     }

//     pub struct Bounds {
//         max_size: usize,
//         is_fixed_size: bool
//     }

//     pub struct Vec<B: Storable> {
//         b: B,
//     }

//     impl<B: Storable> Vec<B> {
//         const IS_FIXED_SIZE: Bounds = {
//             match B::BOUND {
//                 Bound::Unbounded => panic!("should be bounded"),
//                 Bound::Bounded { max_size, is_fixed_size } => Bounds {
//                     max_size: max_size as usize,
//                     is_fixed_size
//                 },
//             }
//         };

//         pub fn new() -> Self {
//             let _ = Self::IS_FIXED_SIZE;

//             todo!()
//         }

//     }

//     struct Unbounded {}

//     impl Storable for Unbounded {
//         const BOUND: Bound = Bound::Unbounded{};
//     }

//     struct Bounded {}

//     impl Storable for Bounded {
//         const BOUND: Bound = Bound::Bounded {
//             max_size: 32,
//             is_fixed_size: false,
//         };
//     }

//     #[test]
//     fn should_not_compile() {
//         // This compiles
//         let vec = Vec::<Bounded>::new();
//         // While this doesn't
//         let vec = Vec::<Unbounded>::new();
//     }

// }
