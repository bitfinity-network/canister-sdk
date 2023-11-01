use std::path::{Path, PathBuf};

use dfinity_stable_structures::Memory;
use parking_lot::RwLock;

use super::error::MemMapResult;
use super::memory_mapped_file::MemoryMappedFile;
use crate::memory::MemoryManager;

const WASM_PAGE_SIZE_IN_BYTES: u64 = 65536;

pub struct MemoryMappedFileMemoryManager {
    base_path: PathBuf,
    is_persistent: bool,
}

impl MemoryMappedFileMemoryManager {
    pub fn new(base_path: PathBuf, is_persistent: bool) -> Self {
        Self {
            base_path,
            is_persistent,
        }
    }
}

impl MemoryManager<MemoryMappedFileMemory, &str> for MemoryMappedFileMemoryManager {
    fn get(&self, id: &str) -> MemoryMappedFileMemory {
        get(&self.base_path, self.is_persistent, id)
    }
}

impl MemoryManager<MemoryMappedFileMemory, &Path> for MemoryMappedFileMemoryManager {
    fn get(&self, id: &Path) -> MemoryMappedFileMemory {
        get(&self.base_path, self.is_persistent, id)
    }
}

impl MemoryManager<MemoryMappedFileMemory, u8> for MemoryMappedFileMemoryManager {
    fn get(&self, id: u8) -> MemoryMappedFileMemory {
        get(&self.base_path, self.is_persistent, id.to_string())
    }
}

fn get<T: AsRef<Path>>(base_path: &Path, is_persistent: bool, id: T) -> MemoryMappedFileMemory {
    let file_path = base_path.join(id.as_ref());
    let file_path = file_path
        .to_str()
        .unwrap_or_else(|| panic!("Cannot extract path from {}", file_path.display()));
    MemoryMappedFileMemory::new(file_path.to_owned(), is_persistent).unwrap_or_else(|_| {
        panic!(
            "failed to initialize MemoryMappedFileMemory with path: {}",
            file_path
        )
    })
}

pub struct MemoryMappedFileMemory(RwLock<MemoryMappedFile>);

impl MemoryMappedFileMemory {
    pub fn new(path: String, is_persistent: bool) -> MemMapResult<Self> {
        Ok(Self(RwLock::new(MemoryMappedFile::new(
            path,
            is_persistent,
        )?)))
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
