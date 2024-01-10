use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dfinity_stable_structures::Memory;
use parking_lot::{RwLock, RwLockReadGuard};

use super::error::{MemMapError, MemMapResult};
use super::memory_mapped_file::MemoryMappedFile;
use crate::memory::MemoryManager;

const WASM_PAGE_SIZE_IN_BYTES: u64 = 65536;

/// When creating mapping we reserve at once 1 TB of address space.
/// This doesn't allocate any resources (except of address space which is not a problem for x64)
/// but allows skip remapping/flushing when the file size grows.
const DEFAULT_MEM_MAP_RESERVED_LENGTH: u64 = 1 << 40;

/// Memory manager that uses one memory mapped filer per one memory id.
pub struct MemoryMappedFileMemoryManager {
    base_path: PathBuf,
    is_persistent: bool,
    created_memory_resources: RwLock<BTreeMap<PathBuf, MemoryMappedFileMemory>>,
    file_reserved_length: u64,
}

impl MemoryMappedFileMemoryManager {
    /// Create new file manager that uses `base_path` folder.
    /// If `is_persistent` is set to false all the files will be removed on drop.
    pub fn new(base_path: PathBuf, is_persistent: bool) -> Self {
        Self {
            base_path,
            is_persistent,
            created_memory_resources: Default::default(),
            file_reserved_length: DEFAULT_MEM_MAP_RESERVED_LENGTH,
        }
    }

    /// Set reserved length for each memory-mapped file.
    pub fn with_reserved_length(mut self, file_reserved_length: u64) -> Self {
        self.file_reserved_length = file_reserved_length;

        self
    }

    /// Flush and save the memory-mapped files to the given path.
    /// Note that this function should be executed at the point when the stable storage state in consistent in order
    /// to save a consistent backup.
    pub fn save_copies_to(&self, path: impl AsRef<Path>) -> Result<(), MemMapError> {
        let created_memory_resources = self.created_memory_resources.read();
        // Acquire rad lock on all memories to guarantee no write actions happen during the backup.
        let locks = created_memory_resources
            .iter()
            .map(|(file_path, memory)| (file_path, memory.read_lock()))
            .collect::<Vec<_>>();
        for (file_path, memory) in locks {
            let file_name = file_path
                .file_name()
                .ok_or(MemMapError::InvalidSourceFileName)?;
            let new_path = path.as_ref().join(
                file_name
                    .to_str()
                    .ok_or(MemMapError::InvalidSourceFileName)?,
            );

            memory.save_copy(new_path)?;
        }

        Ok(())
    }

    fn get_impl(&self, id: impl AsRef<Path>) -> MemoryMappedFileMemory {
        let mut created_memory_resources = self.created_memory_resources.write();
        let file_path = self.base_path.join(id.as_ref());
        match created_memory_resources.entry(file_path) {
            Entry::Vacant(entry) => {
                let file_path = entry.key().to_str().unwrap_or_else(|| {
                    panic!("Cannot extract path from {}", entry.key().display())
                });
                let result = MemoryMappedFileMemory::new(
                    file_path.to_owned(),
                    self.file_reserved_length,
                    self.is_persistent,
                )
                .unwrap_or_else(|_| {
                    panic!(
                        "failed to initialize MemoryMappedFileMemory with path: {}",
                        file_path
                    )
                });

                entry.insert(result.clone());

                result
            }
            Entry::Occupied(entry) => entry.get().clone(),
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
        self.get_impl(id.to_string())
    }
}

#[derive(Clone)]
pub struct MemoryMappedFileMemory(Arc<RwLock<MemoryMappedFile>>);

impl MemoryMappedFileMemory {
    pub fn new(path: String, reserved_length: u64, is_persistent: bool) -> MemMapResult<Self> {
        Ok(Self(Arc::new(RwLock::new(MemoryMappedFile::new(
            path,
            reserved_length,
            is_persistent,
        )?))))
    }

    pub fn set_is_persistent(&self, is_persistent: bool) {
        self.0.write().set_is_persistent(is_persistent)
    }

    pub(super) fn read_lock(&self) -> RwLockReadGuard<'_, MemoryMappedFile> {
        self.0.read()
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
