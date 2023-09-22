/// By default we use chunk size equal to the default page size.
/// Since our structures are usually pretty small it doesn't seem
/// that we will benefit from using huge page size (2 MB or 1 GB)
pub const CHUNK_SIZE: u64 = 4096;
/// When creating mapping we reserve at once 1 TB of address space.
/// This doesn't allocate any resources (except of address space which is not a problem for x64)
/// but allows skip remapping/flushing when the file size grows.
pub const MEM_MAP_RESERVED_LENGTH: u64 = 1 << 40;