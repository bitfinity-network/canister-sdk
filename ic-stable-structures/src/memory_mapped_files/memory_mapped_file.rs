use std::fs::{copy, remove_file, File, OpenOptions};
use std::path::Path;

use memmap2::{MmapMut, MmapOptions};

use super::error::{MemMapError, MemMapResult};

/// By default we use chunk size equal to the default page size.
/// Since our structures are usually pretty small it doesn't seem
/// that we will benefit from using huge page size (2 MB or 1 GB)
const PAGE_SIZE: usize = 4096;

/// Memory mapped file implementation.
/// If `is_persistent` flag is true then after the
/// structure is dropped all the changes are saved to file.
pub(super) struct MemoryMappedFile {
    file: File,
    path: String,
    length: usize,
    max_length: usize,
    is_persistent: bool,
    mapping: MmapMut,
}

impl MemoryMappedFile {
    /// Preconditions: file under the `path` should not be modified from any other place
    /// in this or different process.
    /// `max_length` is used to reserve the memory address space to allow resizing the memory
    /// without flushing data and re-mapping it again.
    pub fn new(path: String, max_length: usize, is_persistent: bool) -> MemMapResult<Self> {
        if !is_persistent {
            _ = remove_file(&path);
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .read(true)
            .open(&path)?;
        let length = file.metadata()?.len() as usize;

        let mut mmap_opts = MmapOptions::new();
        // Safety: function preconditions should guarantee the safety of the operation:
        // mapping to a file is safe if the file isn't modified concurrently by this and other processes.
        let mapping = unsafe { mmap_opts.len(max_length).map_mut(&file) }?;

        Ok(Self {
            file,
            path,
            is_persistent,
            max_length,
            length,
            mapping,
        })
    }

    /// Returns the current length in bytes
    pub fn len(&self) -> usize {
        self.length
    }

    /// Resize the memory mapped file.
    /// `new_length` should be `PAGE_SIZE` multiple.
    /// If `new_length` is less or equal than the current length
    /// nothing happens.
    pub fn resize(&mut self, new_length: usize) -> MemMapResult<usize> {
        if new_length % PAGE_SIZE != 0 {
            return Err(MemMapError::SizeShouldBePageSizeMultiple);
        }

        if new_length < self.length {
            return Ok(self.length);
        }

        if new_length > self.max_length {
            return Err(MemMapError::OutOfAddressSpace {
                claimed: new_length,
                limit: self.max_length,
            });
        }

        // There is no need to remap after changing the size
        self.file.set_len(new_length as u64)?;
        self.length = new_length;

        Ok(self.length)
    }

    /// Read data starting at `offset` to the given buffer.
    pub fn read(&self, offset: usize, dst: &mut [u8]) -> MemMapResult<()> {
        if offset + dst.len() > self.len() {
            return Err(MemMapError::AccessOutOfBounds);
        }

        dst.copy_from_slice(&self.mapping[offset..offset + dst.len()]);

        Ok(())
    }

    /// Write data from `src` to the memory starting at `offset`.
    pub fn write(&mut self, offset: usize, src: &[u8]) -> MemMapResult<()> {
        if offset + src.len() > self.len() {
            return Err(MemMapError::AccessOutOfBounds);
        }

        self.mapping[offset..offset + src.len()].copy_from_slice(src);

        Ok(())
    }

    /// Fill range with zeros.
    pub fn zero_range(&mut self, offset: usize, count: usize) -> MemMapResult<()> {
        if offset + count > self.length {
            return Err(MemMapError::AccessOutOfBounds);
        }

        self.mapping[offset..(offset + count)].fill(0);

        Ok(())
    }

    /// Flush all the changes to the underlying file.
    pub fn flush(&self) -> MemMapResult<()> {
        self.mapping.flush()?;

        Ok(())
    }

    /// Save the copy to a file at the specified path.
    pub fn save_copy(&self, path: impl AsRef<Path>) -> MemMapResult<()> {
        self.flush()?;
        copy(&self.path, path)?;

        Ok(())
    }

    /// Set `is_persistent` flag.
    pub fn set_is_persistent(&mut self, is_persistent: bool) {
        self.is_persistent = is_persistent;
    }
}

impl Drop for MemoryMappedFile {
    fn drop(&mut self) {
        if self.is_persistent {
            self.flush().expect("failed to flush data to file")
        } else {
            _ = remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use tempfile::NamedTempFile;

    use super::*;

    /// Default max length. It cannot be less than a page size.
    const DEFAULT_MAX_LENGTH: usize = PAGE_SIZE;

    fn with_temp_file(func: impl FnOnce(String)) {
        let file = NamedTempFile::new().unwrap();
        let path = file.into_temp_path();

        func(path.to_str().unwrap().to_owned())
    }

    #[test]
    fn should_create_flush_memory_file() {
        with_temp_file(|path| {
            let mut file_memory = MemoryMappedFile::new(path, DEFAULT_MAX_LENGTH, true).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();
            file_memory.flush().unwrap();
        })
    }

    #[test]
    fn should_read_write_first_chunk() {
        with_temp_file(|path| {
            let mut file_memory = MemoryMappedFile::new(path, DEFAULT_MAX_LENGTH, true).unwrap();
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
            let mut file_memory = MemoryMappedFile::new(path, DEFAULT_MAX_LENGTH, true).unwrap();
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
            let mut file_memory = MemoryMappedFile::new(path, DEFAULT_MAX_LENGTH, true).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();

            assert!(matches!(
                file_memory.read(0, &mut [0; PAGE_SIZE + 1]),
                Err(MemMapError::AccessOutOfBounds)
            ));
            assert!(matches!(
                file_memory.read(1, &mut [0; PAGE_SIZE]),
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
            let mut file_memory = MemoryMappedFile::new(path, DEFAULT_MAX_LENGTH, true).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();

            assert!(matches!(
                file_memory.write(0, &[0; PAGE_SIZE + 1]),
                Err(MemMapError::AccessOutOfBounds)
            ));
            assert!(matches!(
                file_memory.write(1, &[0; PAGE_SIZE]),
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
            let mut file_memory = MemoryMappedFile::new(path, PAGE_SIZE * 5, true).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();
            assert_eq!(file_memory.len(), PAGE_SIZE);

            // Fill first chunk
            let slice = &mut [42; PAGE_SIZE];
            file_memory.write(0, slice).unwrap();
            slice.fill(0);
            file_memory.read(0, slice).unwrap();
            assert_eq!(slice, &[42; PAGE_SIZE]);

            file_memory.resize(PAGE_SIZE * 2).unwrap();
            assert_eq!(file_memory.len(), PAGE_SIZE * 2);

            // Fill second chunk
            slice.fill(43);
            file_memory.write(PAGE_SIZE, slice).unwrap();

            let slice = &mut [0; (PAGE_SIZE * 2)];
            file_memory.read(0, slice).unwrap();

            assert_eq!(
                slice,
                &[[42; PAGE_SIZE], [43; PAGE_SIZE]].concat()[..]
            )
        })
    }

    #[test]
    fn should_check_reserved_address_length() {
        with_temp_file(|path| {
            let reserved_size = PAGE_SIZE * 5;
            let mut file_memory = MemoryMappedFile::new(path, reserved_size, true).unwrap();
            let claimed_size = PAGE_SIZE * 6;
            let result = file_memory.resize(claimed_size).unwrap_err();
            assert!(
                matches!(result, MemMapError::OutOfAddressSpace { claimed, limit } if claimed == claimed_size && limit == reserved_size)
            );
        })
    }

    fn create_data() -> Vec<u8> {
        (0..PAGE_SIZE).map(|i| (i % u8::MAX as usize) as u8).collect()
    }

    fn check_data(data: &[u8]) {
        for (i, item) in data.iter().enumerate() {
            assert_eq!(*item, (i % u8::MAX as usize) as u8);
        }
    }

    #[test]
    fn should_flush() {
        with_temp_file(|path| {
            let mut file_memory =
                MemoryMappedFile::new(path.clone(), DEFAULT_MAX_LENGTH, true).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();

            let mut data = create_data();
            file_memory.write(0, &data).unwrap();

            file_memory.flush().unwrap();

            let mut file = File::open(&path).unwrap();
            data.fill(0);
            file.read_exact(&mut data).unwrap();
            check_data(&data);
        });
    }

    #[test]
    fn should_copy_file() {
        with_temp_file(|path| {
            let mut file_memory =
                MemoryMappedFile::new(path.clone(), DEFAULT_MAX_LENGTH, true).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();

            let mut data = create_data();
            file_memory.write(0, &data).unwrap();

            let file = NamedTempFile::new().unwrap();
            let path = file.into_temp_path();
            file_memory.save_copy(&path).unwrap();

            let mut file = File::open(&path).unwrap();
            data.fill(0);
            file.read_exact(&mut data).unwrap();
            check_data(&data);
        });
    }

    #[test]
    fn should_remove_file_for_non_persistent() {
        with_temp_file(|path| {
            let mut file_memory =
                MemoryMappedFile::new(path.clone(), DEFAULT_MAX_LENGTH, false).unwrap();
            file_memory.resize(PAGE_SIZE).unwrap();

            drop(file_memory);

            assert!(!Path::new(&path).exists());
        });
    }
}
