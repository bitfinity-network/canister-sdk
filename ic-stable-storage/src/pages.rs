use std::cell::RefCell;

use super::{RestrictedMemory, StableMemory, RESERVED_PAGE_MEM};
use crate::StableBTreeMap;

const MAX_PAGE_MEM_KEY_SIZE: u32 = 8;
const MAX_PAGE_MEM_VALUE_SIZE: u32 = 0;
const FREE: u8 = u8::MAX;

thread_local! {
    // `StableMemory` is either:
    static PAGES: RefCell<PageMemory> = RefCell::new(
        StableBTreeMap::init(
            RestrictedMemory::new(StableMemory::default(), RESERVED_PAGE_MEM),
            MAX_PAGE_MEM_KEY_SIZE,
            MAX_PAGE_MEM_VALUE_SIZE
        ),
    );
}

type PageMemory = StableBTreeMap<RestrictedMemory<StableMemory>, Vec<u8>, Vec<u8>>;

#[cfg(test)]
fn memory_dump() -> Vec<Vec<u8>> {
    PAGES.with(|pages| pages.borrow().iter().map(|(i, _)| i).collect::<Vec<_>>())
}

/// `Pages` is used to manage `VirtualMemory`.
/// Memory pages belonging to a particular index is tracked via `Pages`.
/// This makes it possible to have two collections of the same type with different indices.
#[derive(Debug, Copy, Clone)]
pub(super) struct Pages(u8);

impl Pages {
    pub(super) fn new(index: u8) -> Self {
        Self(index)
    }

    pub(super) fn reload(&self) {
        PAGES.with(|pages| {
            let replace_with = StableBTreeMap::load(pages.borrow().get_memory());
            pages.replace(replace_with);
        });
    }

    pub(super) fn page_count(&self) -> u64 {
        PAGES.with(|pages| pages.borrow().range(vec![self.0], None).count() as u64)
    }

    pub(super) fn free_pages(&self, indices: impl IntoIterator<Item = Vec<u8>>) {
        PAGES.with(|pages| {
            let mut pages = pages.borrow_mut();
            for index in indices {
                pages.remove(&index);
                let mut key = index;
                key[0] = FREE;
                pages
                    .insert(key, vec![])
                    .expect("insert pages to manager err");
            }
        });
    }

    pub(super) fn forget(self) {
        PAGES.with(|pages| {
            let free_pages = pages
                .borrow()
                .range(vec![self.0], None)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();

            self.free_pages(free_pages);
        })
    }

    pub(super) fn range(&self, skip: usize, take: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
        PAGES.with(|pages| {
            pages
                .borrow()
                .range(vec![self.0], None)
                .skip(skip)
                .take(take)
                .collect()
        })
    }

    pub(super) fn drain_free_pages(&self, count: usize) -> Vec<Vec<u8>> {
        PAGES.with(|pages| {
            let free_pages = pages
                .borrow()
                .range(vec![FREE], None)
                .take(count)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();

            let mut pages = pages.borrow_mut();
            for index in &free_pages {
                pages.remove(index);
            }

            free_pages
        })
    }

    pub(super) fn insert_pages(
        &self,
        mut new_pages: impl Iterator<Item = Vec<u8>>,
    ) -> Result<(), crate::InsertError> {
        PAGES.with(|pages| {
            let mut pages = pages.borrow_mut();
            new_pages.try_for_each(|page| pages.insert(page, vec![]).map(|_| ()))
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Memory;
    use crate::VirtualMemory;
    use std::rc::Rc;

    #[test]
    fn deallocate_memory() {
        let data_memory = StableMemory::default();

        let virtual_memory_0 = VirtualMemory::<_, 0>::init(Rc::clone(&data_memory));
        let virtual_memory_1 = VirtualMemory::<_, 1>::init(Rc::clone(&data_memory));

        assert_eq!(virtual_memory_0.grow(1), 0);
        // virtual_memory_1.pages.borrow_mut().reload();
        assert_eq!(virtual_memory_1.grow(1), 0);

        assert_eq!(virtual_memory_0.grow(1), 1);
        assert_eq!(virtual_memory_1.grow(1), 1);

        assert_eq!(
            memory_dump(),
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
            memory_dump(),
            vec![
                vec![1, 0, 0, 0, 0, 0, 0, 1],
                vec![1, 0, 0, 1, 0, 0, 0, 3],
                vec![255, 0, 0, 0, 0, 0, 0, 0],
                vec![255, 0, 0, 1, 0, 0, 0, 2]
            ]
        );

        assert_eq!(virtual_memory_1.grow(3), 2);
        assert_eq!(
            memory_dump(),
            vec![
                vec![1, 0, 0, 0, 0, 0, 0, 1],
                vec![1, 0, 0, 1, 0, 0, 0, 3],
                vec![1, 0, 0, 2, 0, 0, 0, 0],
                vec![1, 0, 0, 3, 0, 0, 0, 2],
                vec![1, 0, 0, 4, 0, 0, 0, 4],
            ]
        );

        let src_1 = [1; 3 * crate::WASM_PAGE_SIZE as usize];
        virtual_memory_1.write(1, &src_1);
        let mut dst_1 = [0; 3 * crate::WASM_PAGE_SIZE as usize];
        virtual_memory_1.read(1, &mut dst_1);

        assert_eq!(src_1, dst_1);
    }
}
