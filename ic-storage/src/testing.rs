use std::cell::RefCell;
use std::io;

use ic_exports::ic_cdk::api::stable::StableMemoryError;

thread_local! {
    static STORAGE: RefCell<Vec<u8>> = RefCell::new(vec![]);
}

pub fn clear_storage() {
    STORAGE.with(|s| {
        s.borrow_mut().clear();
    });
}

/// Return the page count, not the total bytes in storage.
/// This is how ic_cdk works
pub fn stable_size() -> u32 {
    STORAGE.with(|s| s.borrow().len()) as u32 >> 16
}

pub fn stable_bytes() -> Vec<u8> {
    let size = (stable_size() as usize) << 16;
    let mut vec = Vec::with_capacity(size);

    // This is super dodgy, don't do this.
    // This is copied from the current implementation of stable storage.
    #[allow(clippy::uninit_vec)]
    unsafe {
        vec.set_len(size);
    }

    stable_read(0, vec.as_mut_slice());

    vec
}

pub fn stable_read(offset: u32, buf: &mut [u8]) {
    STORAGE.with(|storage| {
        let offset = offset as usize;
        buf.copy_from_slice(&storage.borrow()[offset..offset + buf.len()]);
    });
}

pub fn stable_write(offset: u32, buf: &[u8]) {
    STORAGE.with(|storage| {
        let offset = offset as usize;
        storage.borrow_mut()[offset..offset + buf.len()].copy_from_slice(buf);
    });
}

pub fn stable_grow(new_pages: u32) -> Result<u32, StableMemoryError> {
    STORAGE.with(|storage| {
        let additional_len = (new_pages << 16) as usize;
        let len = storage.borrow().len();
        match len + additional_len >= u32::MAX as usize {
            false => {
                let previous_size = storage.borrow().len() >> 16;
                storage.borrow_mut().append(&mut vec![0u8; additional_len]);
                Ok(previous_size as u32)
            }
            true => Err(StableMemoryError::OutOfMemory),
        }
    })
}

/// A writer to the stable memory.
///
/// Will attempt to grow the memory as it writes,
/// and keep offsets and total capacity.
pub struct StableWriter {
    /// The offset of the next write.
    offset: usize,

    /// The capacity, in pages.
    capacity: u32,
}

impl Default for StableWriter {
    fn default() -> Self {
        let capacity = stable_size();

        Self {
            offset: 0,
            capacity,
        }
    }
}

impl StableWriter {
    /// Attempts to grow the memory by adding new pages.
    pub fn grow(&mut self, added_pages: u32) -> Result<(), StableMemoryError> {
        let old_page_count = stable_grow(added_pages)?;
        self.capacity = old_page_count + added_pages;
        Ok(())
    }

    /// Writes a byte slice to the buffer.
    ///
    /// The only condition where this will
    /// error out is if it cannot grow the memory.
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, StableMemoryError> {
        if self.offset + buf.len() > ((self.capacity as usize) << 16) {
            self.grow((buf.len() >> 16) as u32 + 1)?;
        }

        stable_write(self.offset as u32, buf);
        self.offset += buf.len();
        Ok(buf.len())
    }
}

impl io::Write for StableWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.write(buf)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Out Of Memory"))
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        // Noop.
        Ok(())
    }
}
