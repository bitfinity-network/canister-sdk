use std::{
    cell::RefCell,
    fs::{remove_file, File, OpenOptions},
};

use ic_exports::ic_cdk::api::stable::WASM_PAGE_SIZE_IN_BYTES;
use ic_exports::stable_structures::Memory;
use memmap2::{MmapMut, MmapOptions};

use super::error::{MemMapError, MemMapResult};

/// By default we use chunk size equal to the default page size.
/// Since our structures are usually pretty small it doesn't seem
/// that we will benefit from using huge page size (2 MB or 1 GB)
const PAGE_SIZE: u64 = 4096;

/// When creating mapping we reserve at once 1 TB of address space.
/// This doesn't allocate any resources (except of address space which is not a problem for x64)
/// but allows skip remapping/flushing when the file size grows.
const MEM_MAP_RESERVED_LENGTH: u64 = 1 << 40;

pub(super) struct MemoryMappedFile {
    file: File,
    path: String,
    length: u64,
    is_permanent: bool,
    mapping: MmapMut,
}

impl MemoryMappedFile {
    /// Preconditions: file under the `path` should not be modified from any other place
    /// in this or different process.
    pub fn new(path: String, is_permanent: bool) -> MemMapResult<Self> {
        if !is_permanent {
            _ =  remove_file(&path);
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .read(true)
            .open(&path)?;
        let length = file.metadata()?.len();

        let mut mmap_opts = MmapOptions::new();
        let mapping = unsafe { mmap_opts.len(MEM_MAP_RESERVED_LENGTH as _).map_mut(&file) }?;

        Ok(Self {
            file,
            path,
            is_permanent,
            length,
            mapping,
        })
    }

    pub fn len(&self) -> u64 {
        self.length
    }

    pub fn resize(&mut self, new_length: u64) -> MemMapResult<u64> {
        assert!(new_length % PAGE_SIZE == 0);

        if new_length < self.length {
            return Ok(self.length);
        }

        if new_length > MEM_MAP_RESERVED_LENGTH {
            return Err(MemMapError::OutOfAddressSpace {
                claimed: new_length,
                limit: MEM_MAP_RESERVED_LENGTH as _,
            });
        }

        // There is no need to remap after changing the size
        self.file.set_len(new_length)?;
        self.length = new_length;

        Ok(self.length)
    }

    pub fn read(&self, offset: u64, dst: &mut [u8]) -> MemMapResult<()> {
        if offset + dst.len() as u64 > self.len() {
            return Err(MemMapError::AccessOutOfBounds);
        }

        dst.copy_from_slice(&self.mapping[offset as usize..offset as usize + dst.len()]);

        Ok(())
    }

    pub fn write(&mut self, offset: u64, src: &[u8]) -> MemMapResult<()> {
        if offset + src.len() as u64 > self.len() {
            return Err(MemMapError::AccessOutOfBounds);
        }

        self.mapping[offset as usize..offset as usize + src.len()].copy_from_slice(src);

        Ok(())
    }

    pub fn zero_range(&mut self, offset: u64, count: u64) -> MemMapResult<()> {
        if offset + count > self.length {
            return Err(MemMapError::AccessOutOfBounds);
        }

        self.mapping[offset as _..(offset + count) as _].fill(0);

        Ok(())
    }

    pub fn flush(&self) -> MemMapResult<()> {
        self.mapping.flush()?;

        Ok(())
    }

    pub fn set_is_permanent(&mut self, is_permanent: bool) {
        self.is_permanent = is_permanent;
    }
}

impl Drop for MemoryMappedFile {
    fn drop(&mut self) {
        if self.is_permanent {
            self.flush().expect("failed to flush data to file")
        } else {
            _ = remove_file(&self.path);
        }
    }
}

thread_local! {
    static MEMORY_MAPPED_FILE_RESOURCE: RefCell<MemoryMappedFile> = {
        let is_permanent = if cfg!(test) { false } else { true };
        RefCell::new(MemoryMappedFile::new("stable_data".to_string(), is_permanent).expect("failed to init memory file"))
    };
}

#[derive(Default)]
pub struct GlobalMemoryMappedFileMemory{}

impl GlobalMemoryMappedFileMemory {
    pub fn set_is_permanent(is_permanent: bool) {
        MEMORY_MAPPED_FILE_RESOURCE.with(|m| m.borrow_mut().set_is_permanent(is_permanent));
    }
}

impl Memory for GlobalMemoryMappedFileMemory {
    fn size(&self) -> u64 {
        MEMORY_MAPPED_FILE_RESOURCE.with(|m| m.borrow().len() / WASM_PAGE_SIZE_IN_BYTES as u64)
    }

    fn grow(&self, pages: u64) -> i64 {
        MEMORY_MAPPED_FILE_RESOURCE.with(|memory| {
            let mut memory = memory.borrow_mut();
            let old_size = memory.len();
            let bytes_to_add = pages * (WASM_PAGE_SIZE_IN_BYTES as u64);
            let new_length = memory
                .resize(old_size + bytes_to_add)
                .expect("failed to resize memory-mapped file");
            memory
                .zero_range(old_size, bytes_to_add)
                .expect("should succeed to zero new memory");

            (new_length / WASM_PAGE_SIZE_IN_BYTES as u64) as i64
        })
    }

    fn read(&self, offset: u64, dst: &mut [u8]) {
        MEMORY_MAPPED_FILE_RESOURCE.with(|memory| {
            memory.borrow()
                .read(offset, dst)
                .expect("invalid memory-mapped file read")
        })
    }

    fn write(&self, offset: u64, src: &[u8]) {
        MEMORY_MAPPED_FILE_RESOURCE.with(|memory| {
            memory.borrow_mut().write(offset, src).expect("invalid memory-mapped file write")
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use tempfile::NamedTempFile;

    use super::*;

    fn with_temp_file(func: impl FnOnce(String)) {
        let file = NamedTempFile::new().unwrap();
        let path = file.into_temp_path();

        func(path.to_str().unwrap().to_owned())
    }

    #[test]
    fn should_create_flush_memory_file() {
        with_temp_file(|path| {
            let mut file_memory = MemoryMappedFile::new(path, true).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();
            file_memory.flush().unwrap();
        })
    }

    #[test]
    fn should_read_write_first_chunk() {
        with_temp_file(|path| {
            let mut file_memory = MemoryMappedFile::new(path, true).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();

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
            let mut file_memory = MemoryMappedFile::new(path, true).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();

            file_memory.write(0, &[1, 2, 3, 4, 5]).unwrap();

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
            let mut file_memory = MemoryMappedFile::new(path, true).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();

            assert!(matches!(
                file_memory.read(0, &mut [0; PAGE_SIZE as usize + 1]),
                Err(MemMapError::AccessOutOfBounds)
            ));
            assert!(matches!(
                file_memory.read(1, &mut [0; PAGE_SIZE as usize]),
                Err(MemMapError::AccessOutOfBounds)
            ));
            assert!(matches!(
                file_memory.read(PAGE_SIZE, &mut [0; 1]),
                Err(MemMapError::AccessOutOfBounds)
            ));
        })
    }

    #[test]
    fn write_out_of_bounds_should_return_error() {
        with_temp_file(|path| {
            let mut file_memory = MemoryMappedFile::new(path, true).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();

            assert!(matches!(
                file_memory.write(0, &[0; PAGE_SIZE as usize + 1]),
                Err(MemMapError::AccessOutOfBounds)
            ));
            assert!(matches!(
                file_memory.write(1, &[0; PAGE_SIZE as usize]),
                Err(MemMapError::AccessOutOfBounds)
            ));
            assert!(matches!(
                file_memory.write(PAGE_SIZE, &[0; 1]),
                Err(MemMapError::AccessOutOfBounds)
            ));
        })
    }

    #[test]
    fn should_expand() {
        with_temp_file(|path| {
            let mut file_memory = MemoryMappedFile::new(path, true).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();
            assert_eq!(file_memory.len(), PAGE_SIZE);

            // Fill first chunk
            let slice = &mut [42; PAGE_SIZE as _];
            file_memory.write(0, slice).unwrap();
            slice.fill(0);
            file_memory.read(0, slice).unwrap();
            assert_eq!(slice, &[42; PAGE_SIZE as _]);

            file_memory.resize(PAGE_SIZE * 2).unwrap();
            assert_eq!(file_memory.len(), PAGE_SIZE * 2);

            // Fill second chunk
            slice.fill(43);
            file_memory.write(PAGE_SIZE, slice).unwrap();

            let slice = &mut [0; (PAGE_SIZE * 2) as _];
            file_memory.read(0, slice).unwrap();

            assert_eq!(
                slice,
                &[[42; PAGE_SIZE as _], [43; PAGE_SIZE as _]].concat()[..]
            )
        })
    }

    #[test]
    fn should_flush() {
        with_temp_file(|path| {
            let mut file_memory = MemoryMappedFile::new(path.clone(), true).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();

            let slice = &mut [0; PAGE_SIZE as _];
            for i in 0..PAGE_SIZE {
                slice[i as usize] = (i % u8::MAX as u64) as _;
            }
            file_memory.write(0, slice).unwrap();

            file_memory.flush().unwrap();

            let mut file = File::open(&path).unwrap();
            slice.fill(0);
            file.read_exact(slice).unwrap();
            for i in 0..PAGE_SIZE {
                assert_eq!(slice[i as usize], (i % u8::MAX as u64) as u8);
            }
        });
    }
}
