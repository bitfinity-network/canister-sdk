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
    use super::{chunk_manager::VirtualMemory, export::stable_structures::RestrictedMemory, *};
    use std::rc::Rc;

    const WASM_PAGE_SIZE: u64 = 65536;

    #[test]
    fn single_entry_grow_size() {
        let manager_memory = StableMemory::default();
        let data_memory = StableMemory::default();
        let virtual_memory = VirtualMemory::init(data_memory, manager_memory, 0);

        assert_eq!(virtual_memory.size(), 0);
        assert_eq!(virtual_memory.grow(10), 0);
        assert_eq!(virtual_memory.size(), 10);

        // The layout should look like this:
        //
        // manager_memory:
        // ------------------------------------------------------------------------- <- Address 0
        //      StableBTreeMap<vec![0, 0, 0, 0, 0, 0, 0, 0], vec![]>
        //                          ↕   \   /    \      /                                    \
        //                          A     B          C
        //  A means virtual_memory index(flag) is 0; B means virtual_memory's page index 0;   \
        //  C means data_memory page index 0;
        //  A & B & C means data_memory page 0 is allocated to virtual_memory's page 0;        \
        //
        //      StableBTreeMap<vec![0, 0, 0, 1, 0, 0, 0, 1], vec![]>                            \
        //                          ↕   \   /    \      /
        //                          A     B          C                                        manager_memory usable page 0
        //  A means virtual_memory index(flag) is 0; B means virtual_memory's page index 1;
        //  C means data_memory page index 1;                                                   /
        //  A & B & C means data_memory page 1 is allocated to virtual_memory's page 1;
        //                                                                                     /
        // ...
        //      StableBTreeMap<vec![0, 0, 0, 9, 0, 0, 0, 9], vec![]>                          /
        //                          ↕   \   /    \      /
        //                          A     B          C                                       /
        //  A means virtual_memory index(flag) is 0; B means virtual_memory's page index 9;
        //  C means data_memory page index 9;                                               /
        //  A & B & C data_memory page 9 is allocated to virtual_memory's page 0;
        // ...
        // ------------------------------------------------------------------------- <- Address 65536
        //                                                                              manager_memory potential pages
        //
        //
        // data_memory:
        // ------------------------------------------------------------------------- <- Address 0
        //      vec![0; 10 * WASM_PAGE_SIZE as usize]                                   data_memory usable pages [0, 10)
        //                                                                              virtual_memory usable pages [0, 10)
        //
        // ------------------------------------------------------------------------- <- Address 65536 * 10
        //                                                                              data_memory potential pages
        //
    }

    #[test]
    fn multiple_entry_grow_size() {
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

        // The layout should look like this:
        //
        // manager_memory:
        // ------------------------------------------------------------------------- <- Address 0
        //      StableBTreeMap<vec![0, 0, 0, 0, 0, 0, 0, 0], vec![]>                                     manager_memory usable page 0
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_0 index(flag) is 0; B means virtual_memory_0's page index 0;
        //  C means data_memory page index 0;
        //  A & B & C means data_memory page 0 is allocated to virtual_memory_0's page 0;
        //
        // ...
        //      StableBTreeMap<vec![0, 0, 0, 4, 0, 0, 0, 4], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_0 index(flag) is 0; B means virtual_memory_0's page index 4;
        //  C means data_memory page index 4;
        //  A & B & C means data_memory page 4 is allocated to virtual_memory_0's page 4;
        //
        //      StableBTreeMap<vec![0, 0, 0, 5, 0, 0, 0, 11], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_0 index(flag) is 0; B means virtual_memory_0's page index 5;
        //  C means data_memory page index 11;
        //  A & B & C means data_memory page 11 is allocated to virtual_memory_0's page 5;
        //
        // ...
        //      StableBTreeMap<vec![0, 0, 0, 11, 0, 0, 0, 17], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_0 index(flag) is 0; B means virtual_memory_0's page index 11;
        //  C means data_memory page index 17;
        //  A & B & C means data_memory page 17 is allocated to virtual_memory_0's page 11;
        //
        //      StableBTreeMap<vec![1, 0, 0, 0, 0, 0, 0, 5], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_1 index(flag) is 1; B means virtual_memory_1's page index 0;
        //  C means data_memory page index 5;
        //  A & B & C means data_memory page 5 is allocated to virtual_memory_1's page 0;
        //
        // ...
        //      StableBTreeMap<vec![1, 0, 0, 5, 0, 0, 0, 10], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_1 index(flag) is 1; B means virtual_memory_1's page index 5;
        //  C means data_memory page index 10;
        //  A & B & C means data_memory page 10 is allocated to virtual_memory_1's page 5;
        //
        //      StableBTreeMap<vec![1, 0, 0, 6, 0, 0, 0, 18], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_1 index(flag) is 1; B means virtual_memory_1's page index 6;
        //  C means data_memory page index 18;
        //  A & B & C means data_memory page 18 is allocated to virtual_memory_1's page 6;
        //
        // ...
        //      StableBTreeMap<vec![1, 0, 0, 13, 0, 0, 0, 25], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_1 index(flag) is 1; B means virtual_memory_1's page index 13;
        //  C means data_memory page index 25;
        //  A & B & C means data_memory page 25 is allocated to virtual_memory_1's page 13;
        // ...
        // ------------------------------------------------------------------------- <- Address 65536
        //                                                                              manager_memory potential pages
        //
        //
        // data_memory:
        // ------------------------------------------------------------------------- <- Address 0
        //      vec![0; 5 * WASM_PAGE_SIZE as usize]                                    data_memory usable pages [0, 5)
        //                                                                              virtual_memory_0 usable pages [0, 5)
        //
        // ------------------------------------------------------------------------- <- Address 65536 * 5
        //      vec![0; 6 * WASM_PAGE_SIZE as usize]                                    data_memory usable pages [5, 11)
        //                                                                              virtual_memory_1 usable pages [0, 6)
        //
        // ------------------------------------------------------------------------- <- Address 65536 * 11
        //      vec![0; 7 * WASM_PAGE_SIZE as usize]                                    data_memory usable pages [11, 18),
        //                                                                              virtual_memory_0 usable pages [5, 12)
        //
        // ------------------------------------------------------------------------- <- Address 65536 * 18
        //      vec![0; 8 * WASM_PAGE_SIZE as usize]                                    data_memory usable pages [18, 26)
        //                                                                              virtual_memory_1 usable pages [6, 14)
        //
        // ------------------------------------------------------------------------- <- Address 65536 * 26
        //                                                                              data_memory potential pages
        //
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn write_without_enough_space() {
        let manager_memory = StableMemory::default();
        let data_memory = StableMemory::default();
        let virtual_memory = VirtualMemory::init(data_memory, manager_memory, 0);

        assert_eq!(virtual_memory.grow(1), 0);
        let src = [1; 1 + WASM_PAGE_SIZE as usize];
        virtual_memory.write(0, &src);
    }

    #[test]
    #[should_panic(expected = "grow failed, which return -1")]
    fn write_without_grow_further() {
        let manager_memory = StableMemory::default();
        let data_memory = RestrictedMemory::new(StableMemory::default(), 0..1);
        let virtual_memory = VirtualMemory::init(data_memory, manager_memory, 0);

        let result = virtual_memory.grow(2);

        // The data_memory's capacity is only 1 page, but virtual_memory try to grow 2 pages. Panic and state changes will be rolled back.
        assert_eq!(result, 0, "grow failed, which return {}", result);
        let src = [1; 1 + WASM_PAGE_SIZE as usize];
        virtual_memory.write(0, &src);
    }

    #[test]
    fn write_multiple_data_to_same_page() {
        let manager_memory = StableMemory::default();
        let data_memory = StableMemory::default();

        let virtual_memory_0 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 0);
        let virtual_memory_1 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 0);

        assert_eq!(virtual_memory_0.grow(1), 0);
        virtual_memory_1.page_range.borrow_mut().reload();

        let src_0 = [1; WASM_PAGE_SIZE as usize];
        let mut dst_0 = [0; WASM_PAGE_SIZE as usize];
        virtual_memory_0.write(0, &src_0);
        virtual_memory_0.read(0, &mut dst_0);
        assert_eq!(src_0, dst_0);

        // Because virtual_memory_0 and virtual_memory_1 use the same virtual_memory index 0, they will all have the same memory pages.
        let src_1 = [2; WASM_PAGE_SIZE as usize];
        let mut dst_1 = [0; WASM_PAGE_SIZE as usize];
        virtual_memory_1.read(0, &mut dst_1);
        assert_eq!(dst_1, src_0);

        virtual_memory_1.write(0, &src_1);
        virtual_memory_1.read(0, &mut dst_1);
        assert_eq!(dst_1, src_1);
    }

    #[test]
    fn write_single_entry_spanning_multiple_pages() {
        let manager_memory = StableMemory::default();
        let data_memory = StableMemory::default();
        let virtual_memory = VirtualMemory::init(data_memory, manager_memory, 0);

        assert_eq!(virtual_memory.grow(3), 0);
        let src = [1; 1 + 2 * WASM_PAGE_SIZE as usize];
        let mut dst = [0; 1 + 2 * WASM_PAGE_SIZE as usize];
        virtual_memory.write(0, &src);
        virtual_memory.read(0, &mut dst);
        assert_eq!(src, dst);
    }

    #[test]
    fn write_multiple_entries_spanning_multiple_pages() {
        let manager_memory = StableMemory::default();
        let data_memory = StableMemory::default();

        let virtual_memory_0 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 0);
        let virtual_memory_1 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 1);
        let virtual_memory_2 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 2);

        assert_eq!(virtual_memory_0.grow(1), 0);
        virtual_memory_1.page_range.borrow_mut().reload();
        assert_eq!(virtual_memory_1.grow(1), 0);
        virtual_memory_2.page_range.borrow_mut().reload();
        assert_eq!(virtual_memory_2.grow(1), 0);

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

        // The layout should look like this:
        //
        // manager_memory:
        // ------------------------------------------------------------------------- <- Address 0
        //      StableBTreeMap<vec![0, 0, 0, 0, 0, 0, 0, 0], vec![]>                                     manager_memory usable page 0
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_0 index(flag) is 0; B means virtual_memory_0's page index 0;
        //  C means data_memory page index 0;
        //  A & B & C means data_memory page 0 is allocated to virtual_memory_0's page 0;
        //
        //      StableBTreeMap<vec![0, 0, 0, 1, 0, 0, 0, 3], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_0 index(flag) is 0; B means virtual_memory_0's page index 1;
        //  C means data_memory page index 3;
        //  A & B & C means data_memory page 3 is allocated to virtual_memory_0's page 1;
        //
        //      StableBTreeMap<vec![0, 0, 0, 2, 0, 0, 0, 6], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_0 index(flag) is 0; B means virtual_memory_0's page index 2;
        //  C means data_memory page index 6;
        //  A & B & C means data_memory page 6 is allocated to virtual_memory_0's page 2;
        //
        //      StableBTreeMap<vec![1, 0, 0, 0, 0, 0, 0, 1], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_1 index(flag) is 1; B means virtual_memory_1's page index 0;
        //  C means data_memory page index 1;
        //  A & B & C means data_memory page 1 is allocated to virtual_memory_1's page 0;
        //
        //      StableBTreeMap<vec![1, 0, 0, 1, 0, 0, 0, 4], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_1 index(flag) is 1; B means virtual_memory_1's page index 1;
        //  C means data_memory page index 4;
        //  A & B & C means data_memory page 4 is allocated to virtual_memory_1's page 1;
        //
        //      StableBTreeMap<vec![1, 0, 0, 2, 0, 0, 0, 7], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_1 index(flag) is 1; B means virtual_memory_1's page index 2;
        //  C means data_memory page index 7;
        //  A & B & C means data_memory page 7 is allocated to virtual_memory_1's page 2;
        //
        //      StableBTreeMap<vec![2, 0, 0, 0, 0, 0, 0, 2], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_2 index(flag) is 2; B means virtual_memory_2's page index 0;
        //  C means data_memory page index 2;
        //  A & B & C means data_memory page 2 is allocated to virtual_memory_2's page 0;
        //
        //      StableBTreeMap<vec![2, 0, 0, 1, 0, 0, 0, 5], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_2 index(flag) is 2; B means virtual_memory_2's page index 1;
        //  C means data_memory page index 5;
        //  A & B & C means data_memory page 5 is allocated to virtual_memory_2's page 1;
        //
        //      StableBTreeMap<vec![2, 0, 0, 2, 0, 0, 0, 8], vec![]>
        //                          ↕   \   /    \      /
        //                          A     B          C
        //  A means virtual_memory_2 index(flag) is 2; B means virtual_memory_2's page index 2;
        //  C means data_memory page index 8;
        //  A & B & C means data_memory page 8 is allocated to virtual_memory_2's page 2;
        // ...
        // ------------------------------------------------------------------------- <- Address 65536
        //                                                                              manager_memory potential pages
        //
        //
        // data_memory:
        // ------------------------------------------------------------------------- <- Address 0
        //      vec![0]                                                                 data_memory usable page 0
        //      vec![1; WASM_PAGE_SIZE as usize - 1]                                    virtual_memory_0 page 0
        //
        // ------------------------------------------------------------------------- <- Address 65536
        //      vec![0]                                                                 data_memory usable page 1
        //      vec![2; WASM_PAGE_SIZE as usize - 1]                                    virtual_memory_1 page 0
        //
        // ------------------------------------------------------------------------- <- Address 65536 * 2
        //      vec![0]                                                                 data_memory usable page 2
        //      vec![3; WASM_PAGE_SIZE as usize - 1]                                    virtual_memory_2 page 0
        //
        // ------------------------------------------------------------------------- <- Address 65536 * 3
        //      vec![1; WASM_PAGE_SIZE as usize]                                        data_memory usable page 3
        //                                                                              virtual_memory_0 page 1
        //
        // ------------------------------------------------------------------------- <- Address 65536 * 4
        //      vec![2; WASM_PAGE_SIZE as usize]                                        data_memory usable page 4
        //                                                                              virtual_memory_1 page 1
        //
        // ------------------------------------------------------------------------- <- Address 65536 * 5
        //      vec![3; WASM_PAGE_SIZE as usize]                                        data_memory usable page 5
        //                                                                              virtual_memory_2 page 1
        //
        // ------------------------------------------------------------------------- <- Address 65536 * 6
        //      vec![1; WASM_PAGE_SIZE as usize - 1]                                    data_memory usable page 6
        //      vec![0]                                                                 virtual_memory_0 page 2
        //
        // ------------------------------------------------------------------------- <- Address 65536 * 7
        //      vec![2; WASM_PAGE_SIZE as usize - 1]                                    data_memory usable page 7
        //      vec![0]                                                                 virtual_memory_1 page 2
        //
        // ------------------------------------------------------------------------- <- Address 65536 * 8
        //      vec![3; WASM_PAGE_SIZE as usize - 1]                                    data_memory usable page 8
        //      vec![0]                                                                 virtual_memory_2 page 2
        //
        // ------------------------------------------------------------------------- <- Address 65536 * 9
        //                                                                              data_memory potential pages;
        //
    }

    #[test]
    fn deallocate_memory() {
        let manager_memory = StableMemory::default();
        let data_memory = StableMemory::default();

        let virtual_memory_0 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 0);
        let virtual_memory_1 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 1);

        assert_eq!(virtual_memory_0.grow(1), 0);
        virtual_memory_1.page_range.borrow_mut().reload();
        assert_eq!(virtual_memory_1.grow(1), 0);

        assert_eq!(virtual_memory_0.grow(1), 1);
        assert_eq!(virtual_memory_1.grow(1), 1);

        assert_eq!(
            StableBTreeMap::<_, Vec<u8>, Vec<u8>>::load(Rc::clone(&manager_memory))
                .iter()
                .map(|(i, _)| i)
                .collect::<Vec<_>>(),
            vec![
                vec![0, 0, 0, 0, 0, 0, 0, 0],
                vec![0, 0, 0, 1, 0, 0, 0, 2],
                vec![1, 0, 0, 0, 0, 0, 0, 1],
                vec![1, 0, 0, 1, 0, 0, 0, 3]
            ]
        );

        assert_eq!(virtual_memory_0.size(), 2);
        assert_eq!(virtual_memory_1.size(), 2);

        virtual_memory_0.forget();

        assert_eq!(
            StableBTreeMap::<_, Vec<u8>, Vec<u8>>::load(Rc::clone(&manager_memory))
                .iter()
                .map(|(i, _)| i)
                .collect::<Vec<_>>(),
            vec![
                vec![1, 0, 0, 0, 0, 0, 0, 1],
                vec![1, 0, 0, 1, 0, 0, 0, 3],
                vec![255, 0, 0, 0, 0, 0, 0, 0],
                vec![255, 0, 0, 1, 0, 0, 0, 2]
            ]
        );

        assert_eq!(virtual_memory_1.grow(3), 2);
        assert_eq!(
            StableBTreeMap::<_, Vec<u8>, Vec<u8>>::load(Rc::clone(&manager_memory))
                .iter()
                .map(|(i, _)| i)
                .collect::<Vec<_>>(),
            vec![
                vec![1, 0, 0, 0, 0, 0, 0, 1],
                vec![1, 0, 0, 1, 0, 0, 0, 3],
                vec![1, 0, 0, 2, 0, 0, 0, 0],
                vec![1, 0, 0, 3, 0, 0, 0, 2],
                vec![1, 0, 0, 4, 0, 0, 0, 4],
            ]
        );

        let src_1 = [1; 3 * WASM_PAGE_SIZE as usize];
        virtual_memory_1.write(1, &src_1);
        let mut dst_1 = [0; 3 * WASM_PAGE_SIZE as usize];
        virtual_memory_1.read(1, &mut dst_1);

        assert_eq!(src_1, dst_1);
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn read_outside_of_memory_range() {
        let manager_memory = StableMemory::default();
        let data_memory = StableMemory::default();
        let virtual_memory = VirtualMemory::init(data_memory, manager_memory, 0);

        let mut dst = [1; WASM_PAGE_SIZE as usize];
        virtual_memory.read(0, &mut dst);
    }

    #[test]
    fn read_single_entry_across_multiple_pages() {
        let manager_memory = StableMemory::default();
        let data_memory = StableMemory::default();
        let virtual_memory = VirtualMemory::init(data_memory, manager_memory, 0);

        assert_eq!(virtual_memory.grow(3), 0);
        let src = [1; 3 * WASM_PAGE_SIZE as usize];
        virtual_memory.write(0, &src);

        let mut dst = [0; 3 * WASM_PAGE_SIZE as usize];
        virtual_memory.read(0, &mut dst);
        assert_eq!(src, dst);
    }

    #[test]
    fn read_multiple_entries_across_multiple_pages() {
        let manager_memory = StableMemory::default();
        let data_memory = StableMemory::default();
        let virtual_memory_0 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 0);
        let virtual_memory_1 =
            VirtualMemory::init(Rc::clone(&data_memory), Rc::clone(&manager_memory), 1);

        assert_eq!(virtual_memory_0.grow(1), 0);
        virtual_memory_1.page_range.borrow_mut().reload();
        assert_eq!(virtual_memory_1.grow(1), 0);

        assert_eq!(virtual_memory_0.grow(1), 1);
        assert_eq!(virtual_memory_1.grow(1), 1);

        let mut dst_0 = [1; 2 * WASM_PAGE_SIZE as usize];
        let mut dst_1 = [1; 2 * WASM_PAGE_SIZE as usize];

        virtual_memory_0.read(0, &mut dst_0);
        virtual_memory_0.read(0, &mut dst_1);

        assert_eq!(dst_0, [0; 2 * WASM_PAGE_SIZE as usize]);
        assert_eq!(dst_1, [0; 2 * WASM_PAGE_SIZE as usize]);
    }
}
