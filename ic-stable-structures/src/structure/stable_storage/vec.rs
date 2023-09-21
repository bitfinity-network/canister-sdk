use dfinity_stable_structures::memory_manager::MemoryId;
use dfinity_stable_structures::{vec, BoundedStorable};

use super::get_memory_by_id;
use crate::structure::VecStructure;
use crate::{Memory, Result};

pub struct StableVec<T: BoundedStorable>(vec::Vec<T, Memory>, MemoryId);

/// A stable analogue of the `std::vec::Vec`:
/// integer-indexed collection of mutable values that is able to grow.
impl<T: BoundedStorable> StableVec<T> {
    /// Creates new `StableVec`
    pub fn new(memory_id: MemoryId) -> Result<Self> {
        Ok(Self(
            vec::Vec::<T, Memory>::init(get_memory_by_id(memory_id))?,
            memory_id,
        ))
    }

    /// Returns iterator over the elements in the vector
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.0.iter()
    }
}

impl<T: BoundedStorable> VecStructure<T> for StableVec<T> {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn clear(&mut self) -> Result<()> {
        self.0 = vec::Vec::<T, Memory>::new(get_memory_by_id(self.1))?;
        Ok(())
    }

    fn len(&self) -> u64 {
        self.0.len()
    }

    fn set(&mut self, index: u64, item: &T) -> Result<()> {
        self.0.set(index, item);
        Ok(())
    }

    fn get(&self, index: u64) -> Option<T> {
        self.0.get(index)
    }

    fn push(&mut self, item: &T) -> Result<()> {
        self.0.push(item).map_err(Into::into)
    }

    fn pop(&mut self) -> Option<T> {
        self.0.pop()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use dfinity_stable_structures::memory_manager::MemoryId;

    #[test]
    fn vec_works() {
        let mut vec = StableVec::<u64>::new(MemoryId::new(0)).unwrap();

        assert!(vec.is_empty());
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.get(0), None);

        vec.push(&1).unwrap();
        assert!(!vec.is_empty());
        assert_eq!(vec.len(), 1);
        assert_eq!(vec.get(0), Some(1));
        assert_eq!(vec.get(1), None);

        vec.push(&2).unwrap();
        assert!(!vec.is_empty());
        assert_eq!(vec.len(), 2);
        assert_eq!(vec.get(0), Some(1));
        assert_eq!(vec.get(1), Some(2));
        assert_eq!(vec.get(2), None);

        assert_eq!(vec.pop(), Some(2));
        assert!(!vec.is_empty());
        assert_eq!(vec.len(), 1);
        assert_eq!(vec.get(0), Some(1));
        assert_eq!(vec.get(1), None);

        assert_eq!(vec.pop(), Some(1));
        assert!(vec.is_empty());
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.get(0), None);

        assert_eq!(vec.pop(), None);
        assert!(vec.is_empty());
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.get(0), None);

        vec.clear().unwrap();
        assert!(vec.is_empty());
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.get(0), None);

        vec.push(&1).unwrap();
        vec.push(&2).unwrap();
        let mut iter = vec.iter();
        assert_eq!(Some(1), iter.next());
        assert_eq!(Some(2), iter.next());
        assert_eq!(None, iter.next());
        drop(iter);

        vec.clear().unwrap();
        assert!(vec.is_empty());
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.get(0), None);
        assert_eq!(None, vec.iter().next());
    }
}
