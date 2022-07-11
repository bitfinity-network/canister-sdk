use stable_structures::{self, Memory, StableBTreeMap};

pub mod chunk_manager;

pub mod export {
    pub use stable_structures;
}

#[cfg(target_arch = "wasm32")]
pub type StableMemory = stable_structures::Ic0StableMemory;
#[cfg(not(target_arch = "wasm32"))]
pub type StableMemory = stable_structures::VectorMemory;
