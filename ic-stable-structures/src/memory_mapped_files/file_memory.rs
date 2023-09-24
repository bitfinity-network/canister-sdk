use std::{fs::{File, OpenOptions}, cell::RefCell};

use ic_exports::ic_cdk::api::stable::WASM_PAGE_SIZE_IN_BYTES;
use ic_exports::stable_structures::Memory;
use memmap2::{MmapMut, MmapOptions};

use super::{error::{MemMapError, MemMapResult}, constant::{CHUNK_SIZE, MEM_MAP_RESERVED_LENGTH}};

pub(super) struct MemoryMappedFile {
    file: File,
    length: u64,
    mapping: MmapMut
}

impl MemoryMappedFile {
    /// Preconditions: file under the `path` should not be modified from any other place
    /// in this or different process.
    pub fn new(path: &str) -> MemMapResult<Self> {
        let file = OpenOptions::new().create(true).append(true).write(true).read(true).open(path)?;
        file.set_len(CHUNK_SIZE)?;

        let mut mmap_opts = MmapOptions::new();
        let mapping = unsafe { mmap_opts.len(MEM_MAP_RESERVED_LENGTH as _).map_mut(&file)}?;

        Ok(Self {
            file,
            length: CHUNK_SIZE,
            mapping
        })
    }

    pub fn len(&self) -> u64 {
        self.length
    }

    pub fn resize(&mut self, new_length: u64) -> MemMapResult<u64> {
        if new_length < self.length {
            return Ok(self.length)
        }

        if new_length > MEM_MAP_RESERVED_LENGTH {
            return Err(MemMapError::OutOfAddressSpace { claimed: new_length, limit: MEM_MAP_RESERVED_LENGTH as _ })
        }

        // There is no need to remap after changing the size
        self.file.set_len(new_length)?;
        self.length = new_length;

        Ok(self.length)
    }

    pub fn read(&self, offset: u64, dst: &mut [u8]) -> MemMapResult<()> {
        if offset + dst.len() as u64 > self.len() {
            return Err(MemMapError::AccessOutOfBounds)
        }

        dst.copy_from_slice(&self.mapping[offset as usize..offset as usize + dst.len()]);

        Ok(())
    }

    pub fn write(&mut self, offset: u64, src: &[u8]) -> MemMapResult<()> {
        if offset + src.len() as u64 > self.len() {
            return Err(MemMapError::AccessOutOfBounds)
        }

        self.mapping[offset as usize..offset as usize + src.len()].copy_from_slice(src);

        Ok(())
    }

    pub fn flush(&self) -> MemMapResult<()> {
        self.mapping.flush()?;

        Ok(())
    }
}

struct MemoryMappedFileMemory(RefCell<MemoryMappedFile>);

impl Memory for MemoryMappedFileMemory {
    fn size(&self) -> u64 {
        self.0.borrow().len()
    }

    fn grow(&self, pages: u64) -> i64 {
        let mut memory = self.0.borrow_mut();
        let old_size = memory.len();
        memory.resize(old_size + pages * (WASM_PAGE_SIZE_IN_BYTES as u64)).expect("failed to resize memory-mapped file") as _
    }

    fn read(&self, offset: u64, dst: &mut [u8]) {
        self.0.borrow().read(offset, dst).expect("invalid memory-mapped file read")
    }

    fn write(&self, offset: u64, src: &[u8]) {
        self.0.borrow_mut().write(offset, src).expect("invalid memory-mapped file write")
    }
}

impl Drop for MemoryMappedFile {
    fn drop(&mut self) {
        self.flush().expect("failed to flush data to file")
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use tempfile::NamedTempFile;

    use super::*;

    fn with_temp_file(func: impl FnOnce(&str)) {
        let file = NamedTempFile::new().unwrap();
        let path = file.into_temp_path();

        func(path.to_str().unwrap())
    }

    #[test]
    fn should_create_flush_memory_file() {
        with_temp_file(|path| {
            let file_memory = MemoryMappedFile::new(path).unwrap();
            file_memory.flush().unwrap();
        })
    }

    #[test]
    fn should_read_write_first_chunk() {
        with_temp_file(|path| {
            let mut file_memory = MemoryMappedFile::new(path).unwrap();

            let slice = &mut [1, 2, 3];
            file_memory.write(0, slice).unwrap();

            slice.fill(0);
            file_memory.read(0, slice).unwrap();

            assert_eq!(slice, &[1, 2, 3]);

            file_memory.write(2, &slice[0..2]).unwrap();

            let slice = &mut [0; 4];
            file_memory.read(0, slice).unwrap();

            assert_eq!(slice, &[1, 2, 1, 2]);
        })
    }

    #[test]
    fn should_read_with_offset() {
        with_temp_file(|path| {
            let mut file_memory = MemoryMappedFile::new(path).unwrap();

            file_memory.write(0, &[1,2, 3, 4, 5]).unwrap();

            let slice = &mut [0; 3];
            file_memory.read(0, slice).unwrap();
            assert_eq!(slice, &[1, 2, 3]);

            slice.fill(0);
            file_memory.read(1, slice).unwrap();
            assert_eq!(slice, &[2, 3, 4]);

            slice.fill(0);
            file_memory.read(2, slice).unwrap();
            assert_eq!(slice, &[3, 4, 5]);
        })
    }

    #[test]
    fn read_out_of_bounds_should_return_error() {
        with_temp_file(|path| {
            let file_memory = MemoryMappedFile::new(path).unwrap();

            assert!(matches!(file_memory.read(0, &mut [0; CHUNK_SIZE as usize + 1]), Err(MemMapError::AccessOutOfBounds)));
            assert!(matches!(file_memory.read(1, &mut [0; CHUNK_SIZE as usize]), Err(MemMapError::AccessOutOfBounds)));
            assert!(matches!(file_memory.read(CHUNK_SIZE, &mut [0; 1]), Err(MemMapError::AccessOutOfBounds)));
        })
    }

    #[test]
    fn write_out_of_bounds_should_return_error() {
        with_temp_file(|path| {
            let mut file_memory = MemoryMappedFile::new(path).unwrap();

            assert!(matches!(file_memory.write(0, &[0; CHUNK_SIZE as usize + 1]), Err(MemMapError::AccessOutOfBounds)));
            assert!(matches!(file_memory.write(1, &[0; CHUNK_SIZE as usize]), Err(MemMapError::AccessOutOfBounds)));
            assert!(matches!(file_memory.write(CHUNK_SIZE, &[0; 1]), Err(MemMapError::AccessOutOfBounds)));
        })
    }

    #[test]
    fn should_expand() {
        with_temp_file(|path| {
            let mut file_memory = MemoryMappedFile::new(path).unwrap();
            assert_eq!(file_memory.len(), CHUNK_SIZE);

            // Fill first chunk
            let slice = &mut [42; CHUNK_SIZE as _];
            file_memory.write(0, slice).unwrap();
            slice.fill(0);
            file_memory.read(0, slice).unwrap();
            assert_eq!(slice, &[42; CHUNK_SIZE as _]);

            file_memory.resize(CHUNK_SIZE * 2).unwrap();
            assert_eq!(file_memory.len(), CHUNK_SIZE * 2);

            // Fill second chunk
            slice.fill(43);
            file_memory.write(CHUNK_SIZE, slice).unwrap();

            let slice = &mut [0; (CHUNK_SIZE * 2) as _];
            file_memory.read(0, slice).unwrap();

            assert_eq!(slice, &[[42; CHUNK_SIZE as _], [43; CHUNK_SIZE as _]].concat()[..])
        })
    }

    #[test]
    fn should_flush() {
        with_temp_file(|path| {
            let mut file_memory = MemoryMappedFile::new(path).unwrap();

            let slice = &mut [0; CHUNK_SIZE as _];
            for i in 0..CHUNK_SIZE {
                slice[i as  usize] = (i % u8::MAX as u64) as _;
            }
            file_memory.write(0, slice).unwrap();

            file_memory.flush().unwrap();

            let mut file = File::open(path).unwrap();
            slice.fill(0);
            file.read_exact(slice).unwrap();
            for i in 0..CHUNK_SIZE {
                assert_eq!(slice[i as  usize], (i % u8::MAX as u64) as u8);
            }
        });
    }
}