use crate::stable::{Memory, StableBTreeMap};
use std::cell::RefCell;
use std::rc::Rc;

const WASM_PAGE_SIZE: u64 = 65536;

/// Manger is used to manage VistualMemory. The specific function is to mark which wasm page in
/// memory belongs to which data, for example, the 0th page belongs to Balance, the 1st page belongs to History, etc.
pub struct Manager<M: Memory> {
    data: StableBTreeMap<M, Vec<u8>, Vec<u8>>,
}

impl<M: Memory + Clone> Manager<M> {
    pub fn init(memory: M) -> Self {
        Self {
            data: StableBTreeMap::init(memory, 4, 0),
        }
    }
}

/// Pack fragmented memory composed of different pages into contiguous memory.
///
/// index stand for different data structures, in the same canister,
/// different data structures should use different indexes.
pub struct VistualMemory<M1: Memory, M2: Memory + Clone> {
    memory: M1,
    page_range: Rc<RefCell<Manager<M2>>>,
    index: u8,
}

impl<M1: Memory, M2: Memory + Clone> VistualMemory<M1, M2> {
    pub fn init(memory: M1, manager_memory: M2, index: u8) -> Self {
        Self {
            memory,
            page_range: Rc::new(RefCell::new(Manager::init(manager_memory))),
            index,
        }
    }

    /// `start` and `end` represents byte index here.
    pub fn base_index(&self, start_byte: u64, end_byte: u64) -> Vec<u64> {
        let start_page = start_byte / WASM_PAGE_SIZE;
        let end_page = end_byte / WASM_PAGE_SIZE;

        self.page_range
            .borrow()
            .data
            .range(vec![self.index], None)
            .skip(start_page as usize)
            .take((end_page - start_page) as usize)
            .map(|(page_index, _)| {
                let page_index = page_index
                    .try_into()
                    .expect("failed to convert Vec<u8> to [u8;4] in base_index");
                self.decode(page_index) as u64
            })
            .collect()
    }

    pub fn encode(&self, key: u32) -> Vec<u8> {
        let mut key = key.to_be_bytes().to_vec();
        assert!(key[0] != 0);
        key[0] = self.index;
        key
    }

    pub fn decode(&self, bytes: [u8; 4]) -> u32 {
        assert!(bytes[0] == self.index);
        let mut bytes = bytes;
        bytes[0] = 0;
        u32::from_be_bytes(bytes)
    }
}

impl<M1: Memory, M2: Memory + Clone> Memory for VistualMemory<M1, M2> {
    fn size(&self) -> u64 {
        self.page_range
            .borrow()
            .data
            .range(vec![self.index], None)
            .count() as u64
    }

    fn grow(&self, pages: u64) -> i64 {
        let size = self.size() as i64;
        let result = self.memory.grow(pages);
        if result == -1 {
            return -1;
        }

        let amount = u32::try_from(result).expect("wasm pages amount too large"); // max pages's amount is 131072(8G)
        let new_amount =
            u32::try_from(result as u64 + pages).expect("new wasm pages amount too large");
        for i in amount..new_amount {
            self.page_range
                .borrow_mut()
                .data
                .insert(self.encode(i), vec![])
                .expect("failed to insert index to manager");
        }
        assert_eq!(self.page_range.borrow().data.len(), new_amount as u64);
        size
    }

    fn read(&self, offset: u64, dst: &mut [u8]) {
        let n = offset + dst.len() as u64;

        if n > self.size() * WASM_PAGE_SIZE {
            panic!("read: out of bounds");
        }

        let offset_position = offset % WASM_PAGE_SIZE;
        let mut offset_position = offset_position as usize;

        let base_pages = self.base_index(offset, n - 1);
        let len = base_pages.len();

        for (i, page_index) in base_pages.iter().enumerate() {
            let start = offset_position + i * WASM_PAGE_SIZE as usize;
            let end = (start + WASM_PAGE_SIZE as usize).min(dst.len());
            let slice = &mut dst[start..end];
            self.memory.read(page_index * WASM_PAGE_SIZE, slice);
            offset_position = 0;
        }
    }

    fn write(&self, offset: u64, src: &[u8]) {
        let n = offset
            .checked_add(src.len() as u64)
            .expect("write: out of bounds");
        if n > self.size() * WASM_PAGE_SIZE {
            panic!("write: out of bounds");
        }

        let offset_postion = offset % WASM_PAGE_SIZE;

        let base_pages = self.base_index(offset, n - 1);
        let len = base_pages.len();
        if len == 0 {
            panic!("write: out of bounds");
        } else if len == 1 {
            self.memory
                .write(base_pages[0] * WASM_PAGE_SIZE + offset_postion, src)
        } else {
            self.memory.write(
                base_pages[0] * WASM_PAGE_SIZE + offset_postion,
                &src[..(WASM_PAGE_SIZE - offset_postion) as usize],
            );
            for (i, value) in base_pages.iter().enumerate() {
                if i != 0 && i != len - 1 {
                    self.memory.write(
                        value * WASM_PAGE_SIZE,
                        &src[(WASM_PAGE_SIZE * i as u64 - offset_postion) as usize
                            ..(WASM_PAGE_SIZE * (i as u64 + 1) - offset_postion) as usize],
                    );
                }
            }
            self.memory.write(
                base_pages[len - 1] * WASM_PAGE_SIZE,
                &src[((len - 1) as u64 * WASM_PAGE_SIZE - offset_postion) as usize..],
            );
        }
    }
}
