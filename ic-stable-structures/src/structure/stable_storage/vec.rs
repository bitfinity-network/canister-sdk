use dfinity_stable_structures::{vec, Memory, Storable};

use crate::structure::VecStructure;
use crate::Result;

pub struct StableVec<T: Storable, M: Memory>(Option<vec::Vec<T, M>>);

/// A stable analogue of the `std::vec::Vec`:
/// integer-indexed collection of mutable values that is able to grow.
impl<T: Storable, M: Memory> StableVec<T, M> {
    /// Creates new `StableVec`
    pub fn new(memory: M) -> Result<Self> {
        Ok(Self(Some(vec::Vec::init(memory)?)))
    }

    /// Returns iterator over the elements in the vector
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.get_inner().iter()
    }

    fn mut_inner(&mut self) -> &mut vec::Vec<T, M> {
        self.0.as_mut().expect("vector is always initialized")
    }

    fn get_inner(&self) -> &vec::Vec<T, M> {
        self.0.as_ref().expect("vector is always initialized")
    }
}

impl<T: Storable, M: Memory> VecStructure<T> for StableVec<T, M> {
    fn is_empty(&self) -> bool {
        self.get_inner().is_empty()
    }

    fn clear(&mut self) -> Result<()> {
        if let Some(vector) = self.0.take() {
            let memory = vector.into_memory();
            self.0 = Some(vec::Vec::new(memory)?);
        }
        Ok(())
    }

    fn len(&self) -> u64 {
        self.get_inner().len()
    }

    fn set(&mut self, index: u64, item: &T) -> Result<()> {
        self.mut_inner().set(index, item);
        Ok(())
    }

    fn get(&self, index: u64) -> Option<T> {
        self.get_inner().get(index)
    }

    fn push(&mut self, item: &T) -> Result<()> {
        self.mut_inner().push(item).map_err(Into::into)
    }

    fn pop(&mut self) -> Option<T> {
        self.mut_inner().pop()
    }
}

#[cfg(test)]
mod tests {

    use dfinity_stable_structures::VectorMemory;

    use super::*;

    #[test]
    fn vec_works() {
        let mut vec = StableVec::<u64, _>::new(VectorMemory::default()).unwrap();

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

    #[should_panic]
    #[test]
    fn vec_unbounded_items() {
        let mut vec = StableVec::<String, _>::new(VectorMemory::default()).unwrap();

        let item = "I am an unbounded item".to_string();
        vec.push(&item).unwrap();
        assert_eq!(Some(item), vec.get(0));
    }
}
