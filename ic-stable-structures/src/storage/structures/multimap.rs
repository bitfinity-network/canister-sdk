use std::collections::HashMap;

use ic_exports::{
    ic_kit::ic,
    stable_structures::{memory_manager::MemoryId, Storable},
    Principal,
};

use crate::{multimap, Iter, Memory, RangeIter, Result};

/// [`StableMultimap`] stores two keys against a single value, making it possible
/// to fetch all values by the root key, or a single value by specifying both keys.
pub struct StableMultimap<K1, K2, V>
where
    K1: Storable,
    K2: Storable,
    V: Storable,
{
    maps: HashMap<Principal, multimap::StableMultimap<Memory, K1, K2, V>>,
    memory_id: MemoryId,
    max_first_key_size: u32,
    max_second_key_size: u32,
    max_value_size: u32,
    empty: multimap::StableMultimap<Memory, K1, K2, V>,
}

impl<K1, K2, V> StableMultimap<K1, K2, V>
where
    K1: Storable,
    K2: Storable,
    V: Storable,
{
    /// Create a new instance of a `StableMultimap`.
    /// All keys and values byte representations should be less then related `..._max_size` arguments.
    pub fn new(
        memory_id: MemoryId,
        max_first_key_size: u32,
        max_second_key_size: u32,
        max_value_size: u32,
    ) -> Self {
        let memory = crate::get_memory_by_id(memory_id);
        let empty = multimap::StableMultimap::new(
            memory,
            max_first_key_size,
            max_second_key_size,
            max_value_size,
        );

        Self {
            maps: HashMap::default(),
            memory_id,
            max_first_key_size,
            max_second_key_size,
            max_value_size,
            empty,
        }
    }

    /// Get a value for the given keys.
    /// If byte representation length of any key exceeds max size, `None` will be returned.
    pub fn get(&self, first_key: &K1, second_key: &K2) -> Option<V> {
        self.get_inner().get(first_key, second_key)
    }

    /// Insert a new value into the map.
    /// Inserting a value with the same keys as an existing value
    /// will result in the old value being overwritten.
    ///
    /// # Errors
    ///
    /// If byte representation length of any key or value exceeds max size, the `Error::ValueTooLarge`
    /// will be returned.
    ///
    /// If stable memory unable to grow, the `Error::OutOfStableMemory` will be returned.
    ///
    /// # Example
    /// ```rust
    /// # use ic_exports::{ic_kit::MockContext, stable_structures::memory_manager::MemoryId};
    /// # use ic_stable_structures::StableMultimap;
    /// # MockContext::new().inject();
    ///
    /// let memory_id = MemoryId::new(0);
    /// let mut map = StableMultimap::new(memory_id, 4, 4, 4);
    ///
    /// map.insert(&0u32, &0u32, &1u32).unwrap();
    /// map.insert(&0u32, &1u32, &2u32).unwrap();
    /// map.insert(&1u32, &1u32, &3u32).unwrap();
    ///
    /// assert_eq!(map.len(), 3);
    ///
    /// ```
    pub fn insert(&mut self, first_key: &K1, second_key: &K2, value: &V) -> Result<()> {
        let canister_id = ic::id();

        // If map for `canister_id` is not initialized, initialize it.
        let map = self.maps.entry(canister_id).or_insert_with(|| {
            let memory = crate::get_memory_by_id(self.memory_id);
            multimap::StableMultimap::new(
                memory,
                self.max_first_key_size,
                self.max_second_key_size,
                self.max_value_size,
            )
        });

        map.insert(first_key, second_key, value)
    }

    /// Remove a specific value and return it.
    ///
    /// # Errors
    ///
    /// If byte representation length of any key exceeds max size, the `Error::ValueTooLarge`
    /// will be returned.
    ///
    /// # Example
    /// ```rust
    /// # use ic_exports::{ic_kit::MockContext, stable_structures::memory_manager::MemoryId};
    /// # use ic_stable_structures::StableMultimap;
    /// # MockContext::new().inject();
    ///
    /// let memory_id = MemoryId::new(0);
    /// let mut map = StableMultimap::new(memory_id, 4, 4, 4);
    ///
    /// map.insert(&0u32, &0u32, &1u32).unwrap();
    /// map.insert(&0u32, &1u32, &2u32).unwrap();
    /// map.insert(&1u32, &1u32, &3u32).unwrap();
    ///
    /// assert_eq!(map.remove(&0, &1).unwrap(), Some(2));
    /// assert_eq!(map.get(&0, &1), None);
    /// assert_eq!(map.len(), 2);
    ///
    /// ```
    pub fn remove(&mut self, first_key: &K1, second_key: &K2) -> Result<Option<V>> {
        self.get_inner_mut().remove(first_key, second_key)
    }

    /// Remove all values for the partial key
    ///
    /// # Errors
    ///
    /// If byte representation length of `first_key` exceeds max size, the `Error::ValueTooLarge`
    /// will be returned.
    ///
    /// # Example
    /// ```rust
    /// # use ic_exports::{ic_kit::MockContext, stable_structures::memory_manager::MemoryId};
    /// # use ic_stable_structures::StableMultimap;
    /// # MockContext::new().inject();
    ///
    /// let memory_id = MemoryId::new(0);
    /// let mut map = StableMultimap::new(memory_id, 4, 4, 4);
    ///
    /// map.insert(&0u32, &0u32, &1u32).unwrap();
    /// map.insert(&0u32, &1u32, &2u32).unwrap();
    /// map.insert(&1u32, &1u32, &3u32).unwrap();
    ///
    /// map.remove_partial(&0).unwrap();
    ///
    /// assert_eq!(map.get(&0, &0), None);
    /// assert_eq!(map.get(&0, &1), None);
    /// assert_eq!(map.get(&1, &1), Some(3));
    /// assert_eq!(map.len(), 1);
    ///
    /// ```
    pub fn remove_partial(&mut self, first_key: &K1) -> Result<()> {
        self.get_inner_mut().remove_partial(first_key)
    }

    /// Get a range of key value pairs based on the root key.
    ///
    /// # Errors
    ///
    /// If byte representation length of `first_key` exceeds max size, the `Error::ValueTooLarge`
    /// will be returned.
    ///
    /// # Example
    /// ```rust
    /// # use ic_exports::{ic_kit::MockContext, stable_structures::memory_manager::MemoryId};
    /// # use ic_stable_structures::StableMultimap;
    /// # MockContext::new().inject();
    ///
    /// let memory_id = MemoryId::new(0);
    /// let mut map = StableMultimap::new(memory_id, 4, 4, 4);
    ///
    /// map.insert(&0u32, &0u32, &1u32).unwrap();
    /// map.insert(&0u32, &1u32, &2u32).unwrap();
    /// map.insert(&1u32, &1u32, &3u32).unwrap();
    ///
    /// let mut range = map.range(&0).unwrap();
    /// assert_eq!(range.next(), Some((0, 1)));
    /// assert_eq!(range.next(), Some((1, 2)));
    /// assert_eq!(range.next(), None);
    /// ```
    pub fn range(&self, first_key: &K1) -> Result<RangeIter<Memory, K2, V>> {
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
    /// let mut map = StableMultimap::new(memory_id, 4, 4, 4);
    ///
    /// map.insert(&0u32, &0u32, &1u32).unwrap();
    /// map.insert(&0u32, &1u32, &2u32).unwrap();
    /// map.insert(&1u32, &1u32, &3u32).unwrap();
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

    /// Items count.
    pub fn len(&self) -> usize {
        self.get_inner().len()
    }

    /// Is map empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn get_inner(&self) -> &multimap::StableMultimap<Memory, K1, K2, V> {
        let canister_id = ic::id();
        self.maps.get(&canister_id).unwrap_or(&self.empty)
    }

    fn get_inner_mut(&mut self) -> &mut multimap::StableMultimap<Memory, K1, K2, V> {
        let canister_id = ic::id();
        self.maps.get_mut(&canister_id).unwrap_or(&mut self.empty)
    }
}