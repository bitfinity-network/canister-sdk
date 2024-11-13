use std::{
    cell::{Ref, RefCell},
    fmt,
};

use dfinity_stable_structures::Memory;

pub const WASM_PAGE_SIZE_IN_BYTES: u64 = 64 * 1024; // 64KB

#[derive(Default)]
pub struct DefaultMemoryImpl {
    stats: RefCell<MemoryStats>,
    inner: dfinity_stable_structures::DefaultMemoryImpl,
}

impl DefaultMemoryImpl {
    pub fn stats(&self) -> Ref<'_, MemoryStats> {
        self.stats.borrow()
    }
}

impl Memory for DefaultMemoryImpl {
    fn size(&self) -> u64 {
        self.inner.size()
    }

    fn grow(&self, pages: u64) -> i64 {
        self.stats.borrow_mut().grow_called(pages);
        self.inner.grow(pages)
    }

    fn read(&self, offset: u64, dst: &mut [u8]) {
        self.stats
            .borrow_mut()
            .reading(bytes_to_pages(offset, dst.len() as _));
        self.inner.read(offset, dst)
    }

    fn write(&self, offset: u64, src: &[u8]) {
        self.stats
            .borrow_mut()
            .writing(bytes_to_pages(offset, src.len() as _));
        self.inner.write(offset, src)
    }
}

fn bytes_to_pages(offset: u64, bytes: u64) -> u64 {
    if bytes == 0 {
        return 0;
    }

    let start_page = offset / WASM_PAGE_SIZE_IN_BYTES;
    let end_page = (offset + bytes).div_ceil(WASM_PAGE_SIZE_IN_BYTES);
    end_page - start_page
}

#[derive(Debug, Default)]
pub struct MemoryStats {
    pages_red: u64,
    pages_written: u64,
    grows: Vec<u64>,
}

impl MemoryStats {
    pub fn grow_called(&mut self, pages: u64) {
        self.grows.push(pages);
    }

    pub fn reading(&mut self, pages: u64) {
        self.pages_red += pages;
    }

    pub fn writing(&mut self, pages: u64) {
        self.pages_written += pages;
    }

    pub fn get_reads(&self) -> u64 {
        self.pages_red
    }

    pub fn get_writes(&self) -> u64 {
        self.pages_written
    }

    pub fn get_acesses(&self) -> u64 {
        self.get_reads() + self.get_writes()
    }

    pub fn get_grows(&self) -> &[u64] {
        &self.grows
    }
}

impl fmt::Display for MemoryStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let max_grow = self.grows.iter().max().copied().unwrap_or_default();
        let total_grow = self.grows.iter().sum::<u64>();
        write!(
            f,
            "Pages red: {}, written: {}, accessed: {}, allocated: {}, max allocation: {}",
            self.get_reads(),
            self.get_writes(),
            self.get_acesses(),
            total_grow,
            max_grow
        )
    }
}
