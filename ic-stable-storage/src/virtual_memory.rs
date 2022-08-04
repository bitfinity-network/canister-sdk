use super::{Pages, WASM_PAGE_SIZE};
use crate::Memory;

// fn get_page_memory() -> StableMemory {
//     PAGE_MEMORY.with(|memory| {
//         let mem: &mut Option<StableMemory> = &mut memory.borrow_mut();
//         match mem {
//             Some(mem) => mem.clone(),
//             None => {
//                 let stable_memory = StableMemory::default();
//                 *mem = Some(stable_memory.clone());
//                 stable_memory
//             }
//         }
//     })
// }

// struct PageMemory(StableBTreeMap<StableMemory, Vec<u8>, u8>);

// impl PageMemory {
//     fn instance() -> Self {
//         panic!()
//         // let memory = get_page_memory();
//         // let map = StableBTreeMap::load(memory, MAX_PAGE_MEM_KEY_SIZE, MAX_PAGE_MEM_VALUE_SIZE);
//         // Self(RefCell::new(map))
//     }

//     fn reload(&mut self) {
//         // let mem = self.0.borrow_mut().get_memory();
//         // self.0 = StableBTreeMap::load(mem);
//     }

//     fn free_page(&mut self, page_index: Vec<u8>) {
//         let storage = &mut self.0;
//         storage.remove(&page_index);
//         let mut key = page_index;
//         key[0] = u8::MAX;
//         storage
//             .insert(key, 0)
//             .expect("insert pages to manager err");
//     }
// }

// /// Manger is used to manage VirtualMemory. The specific function i&s to mark which wasm page in
// /// memory belongs to which data, for example, the 0th page belongs to Balance, the 1st page belongs to History, etc.
// pub struct Pages {
//     memory: PageMemory,
//     page_index: u8,
// }

// impl Pages {
//     fn new(page_index: u8) -> Self {
//         Self {
//             memory: PageMemory::instance(),
//             page_index,
//         }
//     }

//     fn reload(&mut self) {
//         self.memory.reload();
//     }

//     // fn range(&self) -> impl Iterator<Item = (Vec<u8>, u8)> + '_ {
//     //     self.memory.0.borrow().range(vec![self.page_index], None)
//     // }

//     // fn free_pages(&self) -> impl Iterator<Item = (Vec<u8>, u8)> + '_ {
//     //     self.memory.0.range(vec![u8::MAX], None)
//     // }

//     // fn free_page(&mut self, page_index: Vec<u8>) {
//     //     self.memory.free_page(page_index)
//     // }
// }

/// Pack fragmented memory composed of different pages into contiguous memory.
///
/// index stand for different data structures.
/// In the same canister, different data structures should use different indexes.
// #[derive(Clone)]
pub struct VirtualMemory<M1: Memory, const INDEX: u8> {
    memory: M1,
    pages: Pages,
}

impl<M1: Memory, const INDEX: u8> VirtualMemory<M1, INDEX> {
    const ASSERT_VALID: () = assert!(INDEX != u8::MAX);
    pub fn init(memory: M1) -> Self {
        let _ = Self::ASSERT_VALID;

        Self {
            memory,
            pages: Pages::new(INDEX),
        }
    }

    pub fn forget(self) {
        self.pages.forget();
    }

    /// Get a `Vec` of page byte offsets
    /// `start` and `end` represents byte index here.
    fn page_byte_offsets(&self, start_byte: u64, end_byte: u64) -> Vec<u64> {
        let start_page = start_byte / WASM_PAGE_SIZE;
        let end_page = end_byte / WASM_PAGE_SIZE;

        self.pages
            .range(start_page as usize, (end_page - start_page + 1) as usize)
            .into_iter()
            .map(|(page_index, _)| {
                let page_index = page_index
                    .try_into()
                    .expect("failed to convert Vec<u8> to [u8;4] in page_byte_offsets");
                self.decode(page_index).1 as u64
            })
            .map(|page_index| page_index * WASM_PAGE_SIZE)
            .collect::<Vec<_>>()
    }

    fn encode(&self, index: u32, key: u32) -> Vec<u8> {
        let mut index = index.to_be_bytes().to_vec();
        let mut key = key.to_be_bytes().to_vec();
        index.append(&mut key);
        assert!(index[0] == 0);
        index[0] = INDEX;
        index
    }

    fn decode(&self, bytes: [u8; 8]) -> (u32, u32) {
        let mut index: [u8; 4] = bytes[0..4].try_into().expect("slice to array error");
        index[0] = 0;
        let key: [u8; 4] = bytes[4..8].try_into().expect("slice to array error");
        (u32::from_be_bytes(index), u32::from_be_bytes(key))
    }

    // Find the last byte position given an offset and a buffer.
    fn last_byte(&self, offset: u64, buffer: &[u8]) -> u64 {
        let last_byte = offset + buffer.len() as u64 - 1;

        // Get the latest state of page manager after other VirtualMemory modifies it.
        if last_byte >= self.size() * WASM_PAGE_SIZE {
            self.pages.reload();
        }
        if last_byte >= self.size() * WASM_PAGE_SIZE {
            panic!("out of bounds");
        }

        last_byte
    }
}

impl<M1: Memory, const INDEX: u8> Memory for VirtualMemory<M1, INDEX> {
    fn size(&self) -> u64 {
        self.pages.page_count()
    }

    fn grow(&self, pages: u64) -> i64 {
        let size = self.size() as u32;

        let free_pages = self.pages.drain_free_pages(pages as usize);

        // Grow the underlying memory
        let free_page_amount = free_pages.len() as u64;
        let result = self.memory.grow(pages - free_page_amount);
        if result == -1 {
            return -1;
        }

        let begin = result as u32; // max pages's amount is 131072(8G) - 4915200(300G)
        let end = begin + (pages - free_page_amount) as u32;

        // Insert all free page indices
        let pages = free_pages
            .into_iter()
            .flat_map(|key| key.try_into().map(|key| self.decode(key).1))
            .chain(begin..end)
            .enumerate()
            .map(|(i, key)| self.encode(size + i as u32, key));

        self.pages.insert_pages(pages)
            .expect("failed to insert pages");

        size as i64
    }

    fn read(&self, byte_offset: u64, dst: &mut [u8]) {
        let read_to = self.last_byte(byte_offset, dst);

        // Offset position inside a wasm page
        let mut offset_position = byte_offset % WASM_PAGE_SIZE;
        let base_pages = self.page_byte_offsets(byte_offset, read_to);

        let mut start = 0;
        for page in base_pages {
            let end = (start + (WASM_PAGE_SIZE - offset_position) as usize).min(dst.len());
            let slice = &mut dst[start..end];
            start += (WASM_PAGE_SIZE - offset_position) as usize;
            self.memory.read(page + offset_position as u64, slice);
            offset_position = 0;
        }
    }

    // NOTE: `StableBTreeMap` will check size and call `grow` if required
    // so it's not necessary to do that here.
    fn write(&self, byte_offset: u64, src: &[u8]) {
        let write_to = self.last_byte(byte_offset, src);

        // Offset position in wasm page
        let mut offset_position = byte_offset % WASM_PAGE_SIZE;
        let base_pages = self.page_byte_offsets(byte_offset, write_to);

        let mut start = 0;
        for page in base_pages {
            let end = (start + (WASM_PAGE_SIZE - offset_position) as usize).min(src.len());
            let slice = &src[start..end];
            start += (WASM_PAGE_SIZE - offset_position) as usize;
            self.memory.write(page + offset_position as u64, slice);
            offset_position = 0;
        }
    }
}
