use std::collections::BTreeMap;

use ic_exports::ic_kit::ic;
use ic_exports::stable_structures::{memory_manager::MemoryId, BoundedStorable, Vec};
use ic_exports::Principal;

use crate::{Memory, Result};

type InnerVec<T> = Vec<T, Memory>;

pub struct StableVec<T: BoundedStorable> {
    data: BTreeMap<Principal, InnerVec<T>>,
    memory_id: MemoryId,
}

impl<T: BoundedStorable> StableVec<T> {
    pub fn new(memory_id: MemoryId) -> Result<Self> {
        Ok(Self {
            data: Default::default(),
            memory_id,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.get_inner().map_or(true, InnerVec::is_empty)
    }

    pub fn len(&self) -> u64 {
        self.get_inner().map_or(0, InnerVec::len)
    }

    pub fn set(&mut self, index: u64, item: &T) -> Result<()> {
        self.mut_or_create_inner()?.set(index, item);
        Ok(())
    }

    pub fn get(&self, index: u64) -> Option<T> {
        self.get_inner().and_then(|v| v.get(index))
    }

    pub fn push(&mut self, item: &T) -> Result<()> {
        let vec = self.mut_or_create_inner()?;
        vec.push(item).map_err(Into::into)
    }

    pub fn pop(&mut self) -> Option<T> {
        self.mut_inner().and_then(|v| v.pop())
    }

    fn get_inner(&self) -> Option<&InnerVec<T>> {
        let canister_id = ic::id();
        self.data.get(&canister_id)
    }

    fn mut_inner(&mut self) -> Option<&mut InnerVec<T>> {
        let canister_id = ic::id();
        self.data.get_mut(&canister_id)
    }

    fn mut_or_create_inner(&mut self) -> Result<&mut InnerVec<T>> {
        let canister_id = ic::id();
        if let std::collections::btree_map::Entry::Vacant(e) = self.data.entry(canister_id) {
            let vec = InnerVec::new(crate::get_memory_by_id(self.memory_id))?;
            e.insert(vec);
        }

        Ok(self.data.get_mut(&canister_id).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use ic_exports::ic_kit::inject::get_context;
    use ic_exports::ic_kit::{mock_principals, MockContext};
    use ic_exports::stable_structures::memory_manager::MemoryId;

    use super::*;

    fn init_context() {
        MockContext::new().inject();
        set_alice_id();
    }

    fn set_alice_id() {
        get_context().update_id(mock_principals::alice());
    }

    fn set_bob_id() {
        get_context().update_id(mock_principals::bob());
    }

    fn check_values<T: BoundedStorable + Eq + Debug>(
        vec: &StableVec<T>,
        expected_vec: &std::vec::Vec<T>,
    ) {
        assert_eq!(vec.is_empty(), expected_vec.is_empty());
        assert_eq!(vec.len(), expected_vec.len() as u64);

        for i in 0..=vec.len() {
            assert_eq!(vec.get(i).as_ref(), expected_vec.get(i as usize));
        }
    }

    fn check_empty<T: BoundedStorable + Eq + Debug>(vec: &StableVec<T>) {
        check_values(vec, &vec![]);
    }

    #[test]
    fn should_create_empty() {
        init_context();

        let vec = StableVec::<u64>::new(MemoryId::new(0)).unwrap();

        check_empty(&vec);

        set_bob_id();
        check_empty(&vec);
    }

    #[test]
    fn should_push() {
        init_context();

        let mut vec = StableVec::<u64>::new(MemoryId::new(0)).unwrap();
        check_empty(&vec);

        vec.push(&1).unwrap();
        check_values(&vec, &vec![1]);

        set_bob_id();
        check_empty(&vec);

        vec.push(&2).unwrap();
        check_values(&vec, &vec![2]);

        set_alice_id();
        check_values(&vec, &vec![1]);

        vec.push(&3).unwrap();
        check_values(&vec, &vec![1, 3]);
    }

    #[test]
    fn should_pop() {
        init_context();

        let mut vec = StableVec::<u64>::new(MemoryId::new(0)).unwrap();

        vec.push(&1).unwrap();
        vec.push(&2).unwrap();
        vec.push(&3).unwrap();

        set_bob_id();
        vec.push(&4).unwrap();
        vec.push(&5).unwrap();

        set_alice_id();
        assert_eq!(vec.pop(), Some(3));
        check_values(&vec, &vec![1, 2]);

        set_bob_id();
        check_values(&vec, &vec![4, 5]);
        assert_eq!(vec.pop(), Some(5));
        check_values(&vec, &vec![4]);
        assert_eq!(vec.pop(), Some(4));
        check_empty(&vec);
        assert_eq!(vec.pop(), None);
        check_empty(&vec);

        set_alice_id();
        assert_eq!(vec.pop(), Some(2));
        check_values(&vec, &vec![1]);
        assert_eq!(vec.pop(), Some(1));
        check_empty(&vec);
        assert_eq!(vec.pop(), None);
        check_empty(&vec);
    }
}
