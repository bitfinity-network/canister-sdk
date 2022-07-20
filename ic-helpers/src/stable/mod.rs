use stable_structures::{self, Memory, StableBTreeMap};

pub mod chunk_manager;

pub mod export {
    pub use stable_structures;
}

#[cfg(target_arch = "wasm32")]
pub type StableMemory = stable_structures::Ic0StableMemory;
#[cfg(not(target_arch = "wasm32"))]
pub type StableMemory = stable_structures::VectorMemory;

#[cfg(test)]
mod test {
    use super::{chunk_manager::VirtualMemory, *};
    use std::rc::Rc;

    const WASM_PAGE_SIZE: u64 = 65536;

    #[test]
    fn single_grow_size() {
        let manager_memory = StableMemory::default();
        let data_memory = StableMemory::default();
        let virtual_memory = VirtualMemory::init(data_memory, manager_memory, 0);

        assert_eq!(virtual_memory.size(), 0);
        assert_eq!(virtual_memory.grow(10), 0);
        assert_eq!(virtual_memory.size(), 10);
    }

    #[test]
    fn single_write_read() {
        let manager_memory = StableMemory::default();
        let data_memory = StableMemory::default();
        let virtual_memory = VirtualMemory::init(data_memory, manager_memory, 0);

        virtual_memory.grow(2);
        let src = [1; 1 + WASM_PAGE_SIZE as usize];
        let mut dst = [0; 1 + WASM_PAGE_SIZE as usize];
        virtual_memory.write(0, &src);
        virtual_memory.read(0, &mut dst);
        assert_eq!(src, dst);
    }

    #[test]
    fn multiple_grow_size() {
        let manager_memory = StableMemory::default();
        let data_memory = StableMemory::default();

        let virtual_memory_0 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 0);
        let virtual_memory_1 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 1);

        assert_eq!(virtual_memory_0.size(), 0);
        assert_eq!(virtual_memory_1.size(), 0);

        assert_eq!(virtual_memory_0.grow(5), 0);
        virtual_memory_1.page_range.borrow_mut().reload();
        assert_eq!(virtual_memory_1.grow(6), 0);

        assert_eq!(virtual_memory_0.grow(7), 5);
        virtual_memory_1.page_range.borrow_mut().reload();
        assert_eq!(virtual_memory_1.grow(8), 6);

        assert_eq!(virtual_memory_0.size(), 12);
        assert_eq!(virtual_memory_1.size(), 14);
    }

    #[test]
    fn multiple_write_read_ho() {
        let manager_memory = StableMemory::default();
        let data_memory = StableMemory::default();

        let virtual_memory_0 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 0);
        let virtual_memory_1 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 1);
        let virtual_memory_2 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 2);

        virtual_memory_0.grow(1);
        virtual_memory_1.page_range.borrow_mut().reload();
        virtual_memory_1.grow(1);
        virtual_memory_2.page_range.borrow_mut().reload();
        virtual_memory_2.grow(1);

        assert_eq!(virtual_memory_0.grow(1), 1);
        assert_eq!(virtual_memory_1.grow(1), 1);
        assert_eq!(virtual_memory_2.grow(1), 1);

        assert_eq!(virtual_memory_0.grow(1), 2);
        assert_eq!(virtual_memory_1.grow(1), 2);
        assert_eq!(virtual_memory_2.grow(1), 2);

        let src_0 = [1; 3 * WASM_PAGE_SIZE as usize - 2];
        let src_1 = [2; 3 * WASM_PAGE_SIZE as usize - 2];
        let src_2 = [3; 3 * WASM_PAGE_SIZE as usize - 2];

        virtual_memory_0.write(1, &src_0);
        virtual_memory_1.write(1, &src_1);
        virtual_memory_2.write(1, &src_2);

        let mut dst_0 = [0; 3 * WASM_PAGE_SIZE as usize - 2];
        let mut dst_1 = [0; 3 * WASM_PAGE_SIZE as usize - 2];
        let mut dst_2 = [0; 3 * WASM_PAGE_SIZE as usize - 2];

        virtual_memory_0.read(1, &mut dst_0);
        virtual_memory_1.read(1, &mut dst_1);
        virtual_memory_2.read(1, &mut dst_2);

        assert_eq!(src_0, dst_0);
        assert_eq!(src_1, dst_1);
        assert_eq!(src_2, dst_2);
    }
}
