use std::cell::RefCell;
use std::path::Path;

use dfinity_stable_structures::Memory;
use ic_cdk::api::stable::WASM_PAGE_SIZE_IN_BYTES;

use super::error::MemMapResult;
use super::memory_mapped_file::MemoryMappedFile;

pub struct MemoryMappedFileMemory(RefCell<MemoryMappedFile>);

impl MemoryMappedFileMemory {
    pub fn new(path: String, is_permanent: bool) -> MemMapResult<Self> {
        Ok(Self(RefCell::new(MemoryMappedFile::new(
            path,
            is_permanent,
        )?)))
    }

    pub fn set_is_permanent(&self, is_permanent: bool) {
        self.0.borrow_mut().set_is_permanent(is_permanent)
    }

    pub fn save_copy(&self, path: impl AsRef<Path>) -> MemMapResult<()> {
        self.0.borrow().save_copy(path)
    }
}

impl Memory for MemoryMappedFileMemory {
    fn size(&self) -> u64 {
        self.0.borrow().len() / WASM_PAGE_SIZE_IN_BYTES as u64
    }

    fn grow(&self, pages: u64) -> i64 {
        let mut memory = self.0.borrow_mut();
        let old_size = memory.len();
        let bytes_to_add = pages * (WASM_PAGE_SIZE_IN_BYTES as u64);
        let new_length = memory
            .resize(old_size + bytes_to_add)
            .expect("failed to resize memory-mapped file");
        memory
            .zero_range(old_size, bytes_to_add)
            .expect("should succeed to zero new memory");

        (new_length / WASM_PAGE_SIZE_IN_BYTES as u64) as i64
    }

    fn read(&self, offset: u64, dst: &mut [u8]) {
        self.0
            .borrow()
            .read(offset, dst)
            .expect("invalid memory-mapped file read")
    }

    fn write(&self, offset: u64, src: &[u8]) {
        self.0
            .borrow_mut()
            .write(offset, src)
            .expect("invalid memory-mapped file write")
    }
}
