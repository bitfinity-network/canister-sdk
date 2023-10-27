use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dfinity_stable_structures::Memory;
use parking_lot::RwLock;

use super::error::{MemMapResult, MemMapError};
use super::memory_mapped_file::MemoryMappedFile;
use crate::memory::MemoryManager;

const WASM_PAGE_SIZE_IN_BYTES: u64 = 65536;

pub struct MemoryMappedFileMemoryManager {
    base_path: PathBuf,
    is_persistent: bool,
    created_memory_resources: Arc<RwLock<BTreeMap<PathBuf, MemoryMappedFileMemory>>>,
}

impl MemoryMappedFileMemoryManager {
    pub fn new(base_path: PathBuf, is_persistent: bool) -> Self {
        Self {
            base_path,
            is_persistent,
            created_memory_resources: Default::default(),
        }
    }

    pub fn flush_and_save_copies_to(&self, path: impl AsRef<Path>) -> Result<(), MemMapError> {
        let created_memory_resources = self.created_memory_resources.read();
        for (file_path, memory) in created_memory_resources.iter() {
            let file_name = file_path.file_name().ok_or(MemMapError::InvalidSourceFileName)?;
            let new_path = path.as_ref().join(file_name.to_str().ok_or(MemMapError::InvalidSourceFileName)?);

            memory.save_copy(new_path)?;
        }

        Ok(())
    }

    fn get_impl(&self, id: impl AsRef<Path>) -> MemoryMappedFileMemory {
        let mut created_memory_resources = self.created_memory_resources.write();
        let file_path = self.base_path.join(id.as_ref());
        match created_memory_resources.entry(file_path) {
            Entry::Vacant(entry) => {
                let file_path = entry.key()
                    .to_str()
                    .expect(&format!("Cannot extract path from {}", entry.key().display()));
                let result = MemoryMappedFileMemory::new(file_path.to_owned(), self.is_persistent).expect(&format!(
                    "failed to initialize MemoryMappedFileMemory with path: {}",
                    file_path));

                entry.insert(result.clone());

                result
            },
            Entry::Occupied(entry) => {
                entry.get().clone()
            },
        }
    }
}

impl MemoryManager<MemoryMappedFileMemory, &str> for MemoryMappedFileMemoryManager {
    fn get(&self, id: &str) -> MemoryMappedFileMemory {
        self.get_impl(id)
    }
}

impl MemoryManager<MemoryMappedFileMemory, &Path> for MemoryMappedFileMemoryManager {
    fn get(&self, id: &Path) -> MemoryMappedFileMemory {
        self.get_impl(id)
    }
}

impl MemoryManager<MemoryMappedFileMemory, u8> for MemoryMappedFileMemoryManager {
    fn get(&self, id: u8) -> MemoryMappedFileMemory {
        self.get_impl( id.to_string())
    }
}

#[derive(Clone)]
pub struct MemoryMappedFileMemory(Arc<RwLock<MemoryMappedFile>>);

impl MemoryMappedFileMemory {
    pub fn new(path: String, is_persistent: bool) -> MemMapResult<Self> {
        Ok(Self(Arc::new(RwLock::new(MemoryMappedFile::new(
            path,
            is_persistent,
        )?))))
    }

    pub fn set_is_persistent(&self, is_persistent: bool) {
        self.0.write().set_is_persistent(is_persistent)
    }

    pub fn save_copy(&self, path: impl AsRef<Path>) -> MemMapResult<()> {
        self.0.read().save_copy(path)
    }
}

impl Memory for MemoryMappedFileMemory {
    fn size(&self) -> u64 {
        self.0.read().len() / WASM_PAGE_SIZE_IN_BYTES
    }

    fn grow(&self, pages: u64) -> i64 {
        let mut memory = self.0.write();
        let old_size = memory.len();
        let bytes_to_add = pages * WASM_PAGE_SIZE_IN_BYTES;
        let new_length = memory
            .resize(old_size + bytes_to_add)
            .expect("failed to resize memory-mapped file");
        memory
            .zero_range(old_size, bytes_to_add)
            .expect("should succeed to zero new memory");

        (new_length / WASM_PAGE_SIZE_IN_BYTES) as i64
    }

    fn read(&self, offset: u64, dst: &mut [u8]) {
        self.0
            .read()
            .read(offset, dst)
            .expect("invalid memory-mapped file read")
    }

    fn write(&self, offset: u64, src: &[u8]) {
        self.0
            .write()
            .write(offset, src)
            .expect("invalid memory-mapped file write")
    }
}
