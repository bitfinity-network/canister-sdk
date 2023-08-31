use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::BoundedStorable;

use crate::structure::common::multimap;
use crate::{Iter, Memory, RangeIter};

/// `StableMultimap` stores two keys against a single value, making it possible
/// to fetch all values by the root key, or a single value by specifying both keys.
pub struct StableMultimap<K1, K2, V>(multimap::StableMultimap<Memory, K1, K2, V>)
where
    K1: BoundedStorable,
    K2: BoundedStorable,
    V: BoundedStorable;

impl<K1, K2, V> StableMultimap<K1, K2, V>
where
    K1: BoundedStorable,
    K2: BoundedStorable,
    V: BoundedStorable,
{
    /// Create a new instance of a `StableMultimap`.
    /// All keys and values byte representations should be less then related `..._max_size` arguments.
    pub fn new(memory_id: MemoryId) -> Self {
        let memory = crate::get_memory_by_id(memory_id);
        Self(multimap::StableMultimap::new(memory))
    }

    /// Get a value for the given keys.
    /// If byte representation length of any key exceeds max size, `None` will be returned.
    pub fn get(&self, first_key: &K1, second_key: &K2) -> Option<V> {
        self.0.get(first_key, second_key)
    }

    /// Insert a new value into the map.
    /// Inserting a value with the same keys as an existing value
    /// will result in the old value being overwritten.
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `second_key.to_bytes().len() <= K2::MAX_SIZE`
    ///   - `value.to_bytes().len() <= V::MAX_SIZE`
    pub fn insert(&mut self, first_key: &K1, second_key: &K2, value: &V) -> Option<V> {
        self.0.insert(first_key, second_key, value)
    }

    /// Remove a specific value and return it.
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `second_key.to_bytes().len() <= K2::MAX_SIZE`
    pub fn remove(&mut self, first_key: &K1, second_key: &K2) -> Option<V> {
        self.0.remove(first_key, second_key)
    }

    /// Remove all values for the partial key
    ///
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    pub fn remove_partial(&mut self, first_key: &K1) -> bool {
        self.0.remove_partial(first_key)
    }

    /// Get a range of key value pairs based on the root key.
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    pub fn range(&self, first_key: &K1) -> RangeIter<Memory, K1, K2, V> {
        self.0.range(first_key)
    }

    /// Iterator over all items in map.
    pub fn iter(&self) -> Iter<Memory, K1, K2, V> {
        self.0.iter()
    }

    /// Items count.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Is map empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Remove all entries from the map.
    pub fn clear(&mut self) {
        self.0.clear()
    }
}

#[cfg(test)]
mod tests {

    use ic_exports::stable_structures::memory_manager::MemoryId;
    use super::*;

    #[test]
    fn multimap_works() {
        let mut map = StableMultimap::new(MemoryId::new(0));
        assert!(map.is_empty());

        map.insert(&0u32, &0u32, &42u32);
        map.insert(&0u32, &1u32, &84u32);

        map.insert(&1u32, &0u32, &10u32);
        map.insert(&1u32, &1u32, &20u32);

        assert_eq!(map.len(), 4);
        assert_eq!(map.get(&0, &0), Some(42));
        assert_eq!(map.get(&0, &1), Some(84));
        assert_eq!(map.get(&1, &0), Some(10));
        assert_eq!(map.get(&1, &1), Some(20));

        let mut iter = map.iter();
        assert_eq!(iter.next(), Some((0, 0, 42)));
        assert_eq!(iter.next(), Some((0, 1, 84)));
        assert_eq!(iter.next(), Some((1, 0, 10)));
        assert_eq!(iter.next(), Some((1, 1, 20)));
        assert_eq!(iter.next(), None);

        let mut range = map.range(&0);
        assert_eq!(range.next(), Some((0, 42)));
        assert_eq!(range.next(), Some((1, 84)));
        assert_eq!(range.next(), None);

        assert!(map.remove_partial(&0));
        assert!(!map.remove_partial(&0));
        assert_eq!(map.len(), 2);

        assert_eq!(map.remove(&1, &0), Some(10));
        assert_eq!(map.iter().next(), Some((1, 1, 20)));
        assert_eq!(map.len(), 1);
    }

}
