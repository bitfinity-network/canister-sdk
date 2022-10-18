mod storage;

pub use storage::{
    error::Error, get_memory_by_id, structures::StableBTreeMap, structures::StableCell,
    Memory,
};
pub use ic_exports::stable_structures::{
    memory_manager::MemoryId,
    Storable,
};
