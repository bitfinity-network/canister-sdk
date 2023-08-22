use std::collections::HashMap;

use ic_exports::candid::Principal;
use ic_exports::ic_kit::ic;
use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::BoundedStorable;

use crate::{multimap, Iter, Memory, RangeIter};

/// [`StableMultimap`] stores two keys against a single value, making it possible
/// to fetch all values by the root key, or a single value by specifying both keys.
pub struct StableMultimap<K1, K2, V>
where
    K1: BoundedStorable,
    K2: BoundedStorable,
    V: BoundedStorable,
{
    maps: HashMap<Principal, multimap::StableMultimap<Memory, K1, K2, V>>,
    memory_id: MemoryId,
    empty: multimap::StableMultimap<Memory, K1, K2, V>,
}

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
        let empty = multimap::StableMultimap::new(memory);

        Self {
            maps: HashMap::default(),
            memory_id,
            empty,
        }
    }

    /// Get a value for the given keys.
    /// If byte representation length of any key exceeds max size, `None` will be returned.
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `second_key.to_bytes().len() <= K2::MAX_SIZE`
    pub fn get(&self, first_key: &K1, second_key: &K2) -> Option<V> {
        self.get_inner().get(first_key, second_key)
    }

    /// Insert a new value into the map.
    /// Inserting a value with the same keys as an existing value
    /// will result in the old value being overwritten.
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `second_key.to_bytes().len() <= K2::MAX_SIZE`
    ///   - `value.to_bytes().len() <= V::MAX_SIZE`
    ///
    /// # Example
    /// ```rust
    /// # use ic_exports::{ic_kit::MockContext, stable_structures::memory_manager::MemoryId};
    /// # use ic_stable_structures::StableMultimap;
    /// # MockContext::new().inject();
    ///
    /// let memory_id = MemoryId::new(0);
    /// let mut map = StableMultimap::new(memory_id);
    ///
    /// map.insert(&0u32, &0u32, &1u32);
    /// map.insert(&0u32, &1u32, &2u32);
    /// map.insert(&1u32, &1u32, &3u32);
    ///
    /// assert_eq!(map.len(), 3);
    ///
    /// ```
    pub fn insert(&mut self, first_key: &K1, second_key: &K2, value: &V) -> Option<V> {
        let canister_id = ic::id();

        // If map for `canister_id` is not initialized, initialize it.
        let map = self.maps.entry(canister_id).or_insert_with(|| {
            let memory = crate::get_memory_by_id(self.memory_id);
            multimap::StableMultimap::new(memory)
        });

        map.insert(first_key, second_key, value)
    }

    /// Remove a specific value and return it.
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `second_key.to_bytes().len() <= K2::MAX_SIZE`
    ///
    /// # Example
    /// ```rust
    /// # use ic_exports::{ic_kit::MockContext, stable_structures::memory_manager::MemoryId};
    /// # use ic_stable_structures::StableMultimap;
    /// # MockContext::new().inject();
    ///
    /// let memory_id = MemoryId::new(0);
    /// let mut map = StableMultimap::new(memory_id);
    ///
    /// map.insert(&0u32, &0u32, &1u32);
    /// map.insert(&0u32, &1u32, &2u32);
    /// map.insert(&1u32, &1u32, &3u32);
    ///
    /// assert_eq!(map.remove(&0, &1), Some(2));
    /// assert_eq!(map.get(&0, &1), None);
    /// assert_eq!(map.len(), 2);
    ///
    /// ```
    pub fn remove(&mut self, first_key: &K1, second_key: &K2) -> Option<V> {
        self.mut_inner().remove(first_key, second_key)
    }

    /// Remove all values for the partial key
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    ///
    /// # Example
    /// ```rust
    /// # use ic_exports::{ic_kit::MockContext, stable_structures::memory_manager::MemoryId};
    /// # use ic_stable_structures::StableMultimap;
    /// # MockContext::new().inject();
    ///
    /// let memory_id = MemoryId::new(0);
    /// let mut map = StableMultimap::new(memory_id);
    ///
    /// map.insert(&0u32, &0u32, &1u32);
    /// map.insert(&0u32, &1u32, &2u32);
    /// map.insert(&1u32, &1u32, &3u32);
    ///
    /// map.remove_partial(&0);
    ///
    /// assert_eq!(map.get(&0, &0), None);
    /// assert_eq!(map.get(&0, &1), None);
    /// assert_eq!(map.get(&1, &1), Some(3));
    /// assert_eq!(map.len(), 1);
    ///
    /// ```
    pub fn remove_partial(&mut self, first_key: &K1) {
        self.mut_inner().remove_partial(first_key)
    }

    /// Get a range of key value pairs based on the root key.
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    ///
    /// # Example
    /// ```rust
    /// # use ic_exports::{ic_kit::MockContext, stable_structures::memory_manager::MemoryId};
    /// # use ic_stable_structures::StableMultimap;
    /// # MockContext::new().inject();
    ///
    /// let memory_id = MemoryId::new(0);
    /// let mut map = StableMultimap::new(memory_id);
    ///
    /// map.insert(&0u32, &0u32, &1u32);
    /// map.insert(&0u32, &1u32, &2u32);
    /// map.insert(&1u32, &1u32, &3u32);
    ///
    /// let mut range = map.range(&0);
    /// assert_eq!(range.next(), Some((0, 1)));
    /// assert_eq!(range.next(), Some((1, 2)));
    /// assert_eq!(range.next(), None);
    /// ```
    pub fn range(&self, first_key: &K1) -> RangeIter<Memory, K1, K2, V> {
        self.get_inner().range(first_key)
    }

    /// Iterator over all items in map.
    ///
    /// # Example
    /// ```rust
    /// # use ic_exports::{ic_kit::MockContext, stable_structures::memory_manager::MemoryId};
    /// # use ic_stable_structures::StableMultimap;
    /// # MockContext::new().inject();
    ///
    /// let memory_id = MemoryId::new(0);
    /// let mut map = StableMultimap::new(memory_id);
    ///
    /// map.insert(&0u32, &0u32, &1u32);
    /// map.insert(&0u32, &1u32, &2u32);
    /// map.insert(&1u32, &1u32, &3u32);
    ///
    /// let mut iter = map.iter();
    /// assert_eq!(iter.next(), Some((0, 0, 1)));
    /// assert_eq!(iter.next(), Some((0, 1, 2)));
    /// assert_eq!(iter.next(), Some((1, 1, 3)));
    /// assert_eq!(iter.next(), None);
    /// ```
    pub fn iter(&self) -> Iter<Memory, K1, K2, V> {
        self.get_inner().iter()
    }

    /// Item count.
    pub fn len(&self) -> usize {
        self.get_inner().len()
    }

    /// Is map empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Remove all entries from the map.
    pub fn clear(&mut self) {
        self.mut_inner().clear()
    }

    fn get_inner(&self) -> &multimap::StableMultimap<Memory, K1, K2, V> {
        let canister_id = ic::id();
        self.maps.get(&canister_id).unwrap_or(&self.empty)
    }

    fn mut_inner(&mut self) -> &mut multimap::StableMultimap<Memory, K1, K2, V> {
        let canister_id = ic::id();
        self.maps.get_mut(&canister_id).unwrap_or(&mut self.empty)
    }
}

#[cfg(test)]
mod tests {
    use ic_exports::ic_kit::inject::get_context;
    use ic_exports::ic_kit::{mock_principals, MockContext};
    use ic_exports::stable_structures::memory_manager::MemoryId;

    use super::StableMultimap;

    #[test]
    fn map_works() {
        MockContext::new().inject();
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

        map.remove_partial(&0);
        assert_eq!(map.len(), 2);

        assert_eq!(map.remove(&1, &0), Some(10));
        assert_eq!(map.iter().next(), Some((1, 1, 20)));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn two_canisters() {
        MockContext::new()
            .with_id(mock_principals::alice())
            .inject();

        let mut map = StableMultimap::new(MemoryId::new(0));

        map.insert(&0u32, &0u32, &42u32);
        map.insert(&1u32, &0u32, &10u32);

        get_context().update_id(mock_principals::bob());
        map.insert(&0u32, &1u32, &84u32);
        map.insert(&1u32, &1u32, &20u32);

        get_context().update_id(mock_principals::alice());
        assert_eq!(map.get(&0, &0), Some(42));
        assert_eq!(map.len(), 2);

        get_context().update_id(mock_principals::bob());
        assert_eq!(map.get(&1, &1), Some(20));
        assert_eq!(map.len(), 2);
    }
}
