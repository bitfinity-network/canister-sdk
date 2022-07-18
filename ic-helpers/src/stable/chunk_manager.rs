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
#[derive(Clone)]
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

    pub fn base_index(&self, start: u64, end: u64) -> Vec<u64> {
        let start_page = start / WASM_PAGE_SIZE;
        let end_page = end / WASM_PAGE_SIZE;

        let mut result = vec![];
        if start_page < self.size() {
            for (i, val) in self
                .page_range
                .borrow()
                .data
                .range(vec![self.index], None)
                .enumerate()
            {
                if i as u64 >= start_page && i as u64 <= end_page {
                    result.push(
                        self.decode(
                            val.0
                                .try_into()
                                .expect("failed to convert Vec<u8> to [u8;4] in base_index"),
                        ) as u64,
                    );
                }
            }
        }
        result
    }

    pub fn encode(&self, key: u32) -> Vec<u8> {
        let mut key = key.to_be_bytes().to_vec();
        assert!(key[0] == 0);
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

    ///
    fn grow(&self, pages: u64) -> i64 {
        let size = self.size() as i64;
        let result = self.memory.grow(pages);
        if result == -1 {
            return -1;
        }

        let amount = u32::try_from(result).expect("wasm pages amount too large"); // max pages's amount is 131072(8G-300G)
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

    // |--..--|--..--|--offset..--| ,,, |--..--|
    fn read(&self, offset: u64, dst: &mut [u8]) {
        let n = offset
            .checked_add(dst.len() as u64)
            .expect("read: out of bounds");
        if n > self.size() * WASM_PAGE_SIZE {
            panic!("read: out of bounds");
        }
        if n == 0 {
            return;
        }

        let offset_postion = offset % WASM_PAGE_SIZE;

        let base_pages = self.base_index(offset, n - 1);
        let len = base_pages.len();
        if len == 0 {
            panic!("read: out of bounds");
        } else if len == 1 {
            self.memory
                .read(base_pages[0] * WASM_PAGE_SIZE + offset_postion, dst)
        } else {
            let mut first: Vec<u8> = vec![0; (WASM_PAGE_SIZE - offset_postion) as usize];
            self.memory
                .read(base_pages[0] * WASM_PAGE_SIZE + offset_postion, &mut first);

            for (i, value) in base_pages.iter().enumerate() {
                if i != 0 && i != len - 1 {
                    let mut part: Vec<u8> = vec![0; WASM_PAGE_SIZE as usize];
                    self.memory.read(value * WASM_PAGE_SIZE, &mut part);
                    first.extend_from_slice(&part);
                }
            }

            let mut last: Vec<u8> =
                vec![0; dst.len() - (WASM_PAGE_SIZE * (len - 1) as u64 - offset_postion) as usize];
            self.memory.read(
                base_pages[len - 1] * WASM_PAGE_SIZE + offset_postion,
                &mut last,
            );
            first.extend_from_slice(&last);
            dst.copy_from_slice(&first);
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
