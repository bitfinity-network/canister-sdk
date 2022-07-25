use crate::stable::{Memory, StableBTreeMap};
use std::cell::RefCell;
use std::rc::Rc;

const WASM_PAGE_SIZE: u64 = 65536;

/// Manger is used to manage VirtualMemory. The specific function is to mark which wasm page in
/// memory belongs to which data, for example, the 0th page belongs to Balance, the 1st page belongs to History, etc.
pub struct Manager<M: Memory>(StableBTreeMap<M, Vec<u8>, Vec<u8>>);

impl<M: Memory + Clone> Manager<M> {
    pub fn init(memory: M) -> Self {
        Self(StableBTreeMap::init(memory, 8, 0))
    }

    pub(super) fn reload(&mut self) {
        self.0 = StableBTreeMap::load(self.0.get_memory());
    }
}

/// Pack fragmented memory composed of different pages into contiguous memory.
///
/// index stand for different data structures.
/// In the same canister, different data structures should use different indexes.
#[derive(Clone)]
pub struct VirtualMemory<M1: Memory, M2: Memory + Clone> {
    memory: M1,
    pub page_range: Rc<RefCell<Manager<M2>>>,
    index: u8,
}

impl<M1: Memory, M2: Memory + Clone> VirtualMemory<M1, M2> {
    pub fn init(memory: M1, manager_memory: M2, index: u8) -> Self {
        assert_ne!(index, u8::MAX);
        Self {
            memory,
            page_range: Rc::new(RefCell::new(Manager::init(manager_memory))),
            index,
        }
    }

    pub fn forget(self) {
        let temp = self
            .page_range
            .borrow()
            .0
            .range(vec![self.index], None)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();

        for i in temp {
            let storage = &mut self.page_range.borrow_mut().0;
            storage.remove(&i);
            let mut key = i;
            key[0] = u8::MAX;
            storage
                .insert(key, vec![])
                .expect("insert pages to manager err");
        }
    }

    /// Get a `Vec` of page byte offsets
    /// `start` and `end` represents byte index here.
    fn page_byte_offsets(&self, start_byte: u64, end_byte: u64) -> Vec<u64> {
        let start_page = start_byte / WASM_PAGE_SIZE;
        let end_page = end_byte / WASM_PAGE_SIZE;

        self.page_range
            .borrow()
            .0
            .range(vec![self.index], None)
            .skip(start_page as usize)
            .take((end_page - start_page + 1) as usize)
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
        index[0] = self.index;
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
            self.page_range.borrow_mut().reload();
        }
        if last_byte >= self.size() * WASM_PAGE_SIZE {
            panic!("out of bounds");
        }

        last_byte
    }
}

impl<M1: Memory, M2: Memory + Clone> Memory for VirtualMemory<M1, M2> {
    fn size(&self) -> u64 {
        self.page_range
            .borrow()
            .0
            .range(vec![self.index], None)
            .count() as u64
    }

    fn grow(&self, pages: u64) -> i64 {
        let size = self.size() as u32;

        let free_pages = self
            .page_range
            .borrow()
            .0
            .range(vec![u8::MAX], None)
            .take(pages as usize)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();

        let free_page_amount = free_pages.len() as u64;
        let result = self.memory.grow(pages - free_page_amount);
        if result == -1 {
            return -1;
        }

        let storage = &mut self.page_range.borrow_mut().0;

        free_pages.into_iter().enumerate().for_each(|(i, key)| {
            storage.remove(&key);

            let page_index = key
                .try_into()
                .expect("failed to convert Vec<u8> to [u8;4] in page_byte_offsets");
            let page_index = self.decode(page_index).1;
            storage
                .insert(self.encode(size + i as u32, page_index), vec![])
                .expect("insert pages to manager err");
        });

        let begin = result as u32; // max pages's amount is 131072(8G) - 4915200(300G)
        let end = begin + (pages - free_page_amount) as u32;

        (begin..end).enumerate().for_each(|(i, key)| {
            storage
                .insert(
                    self.encode(size + free_page_amount as u32 + i as u32, key),
                    vec![],
                )
                .expect("insert pages to manager err");
        });

        size as i64
    }

    fn read(&self, byte_offset: u64, dst: &mut [u8]) {
        let read_to = self.last_byte(byte_offset, dst);

        // Offset position inside a wasm page
        let mut offset_position = byte_offset % WASM_PAGE_SIZE;
        let base_pages = self.page_byte_offsets(byte_offset, read_to);

        let mut start = 0;
        for page in base_pages {
            let slice = &mut dst[start..];
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
