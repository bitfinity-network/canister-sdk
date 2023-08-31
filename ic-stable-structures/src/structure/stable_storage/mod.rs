use std::cell::RefCell;

use ic_exports::stable_structures::memory_manager::MemoryId;
use ic_exports::stable_structures::{btreemap, cell, log, vec, BoundedStorable, Storable, DefaultMemoryImpl};

use super::common::unbounded::{self, SlicedStorable};
use super::common::multimap;
use crate::{Error, Iter, Memory, RangeIter, Result, MemoryManager};

thread_local! {
    // The memory manager is used for simulating multiple memories. Given a `MemoryId` it can
    // return a memory that can be used by stable structures.
    static MANAGER: RefCell<MemoryManager> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
}

// Return memory by `MemoryId`.
// Each instance of stable structures must have unique `MemoryId`;
pub fn get_memory_by_id(id: MemoryId) -> Memory {
    MANAGER.with(|mng| mng.borrow_mut().get(id))
}

/// Stores value in stable memory, providing `get()/set()` API.
pub struct StableCell<T: Storable>(cell::Cell<T, Memory>);

impl<T: Storable> StableCell<T> {
    /// Create new storage for values with `T` type.
    pub fn new(memory_id: MemoryId, value: T) -> Result<Self> {
        let memory = get_memory_by_id(memory_id);
        let cell = cell::Cell::init(memory, value)?;
        Ok(Self(cell))
    }

    /// Returns reference to value stored in stable memory.
    pub fn get(&self) -> &T {
        self.0.get()
    }

    /// Updates value in stable memory.
    pub fn set(&mut self, value: T) -> Result<()> {
        self.0.set(value)?;
        Ok(())
    }
}

/// Stores key-value data in stable memory.
pub struct StableBTreeMap<K, V>(btreemap::BTreeMap<K, V, Memory>)
where
    K: BoundedStorable + Ord + Clone,
    V: BoundedStorable;

impl<K, V> StableBTreeMap<K, V>
where
    K: BoundedStorable + Ord + Clone,
    V: BoundedStorable,
{
    /// Create new instance of key-value storage.
    pub fn new(memory_id: MemoryId) -> Self {
        let memory = get_memory_by_id(memory_id);
        Self(btreemap::BTreeMap::init(memory))
    }

    /// Return value associated with `key` from stable memory.
    pub fn get(&self, key: &K) -> Option<V> {
        self.0.get(key)
    }

    /// Add or replace value associated with `key` in stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    ///   - `value.to_bytes().len() <= V::MAX_SIZE`
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }

    /// Remove value associated with `key` from stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.0.remove(key)
    }

    /// Iterate over all currently stored key-value pairs.
    pub fn iter(&self) -> btreemap::Iter<'_, K, V, Memory> {
        self.0.iter()
    }

    /// Count of items in the map.
    pub fn len(&self) -> u64 {
        self.0.len()
    }

    /// Is the map empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Remove all entries from the map.
    pub fn clear(&mut self) {
        let inner = &mut self.0;

        let keys: Vec<_> = inner.iter().map(|(k, _)| k).collect();
        for key in keys {
            inner.remove(&key);
        }
    }
}

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

/// Stores list of immutable values in stable memory.
/// Provides only `append()` and `get()` operations.
pub struct StableLog<T: Storable>(Option<log::Log<T, Memory, Memory>>);

impl<T: Storable> StableLog<T> {
    /// Create new storage for values with `T` type.
    pub fn new(index_memory_id: MemoryId, data_memory_id: MemoryId) -> Result<Self> {
        // Method returns Result to be compatible with wasm implementation.

        // Index and data should be stored in different memories.
        assert_ne!(index_memory_id, data_memory_id);

        let index_memory = crate::get_memory_by_id(index_memory_id);
        let data_memory = crate::get_memory_by_id(data_memory_id);

        let inner = log::Log::init(index_memory, data_memory)?;
        Ok(Self(Some(inner)))
    }

    /// Returns reference to value stored in stable memory.
    pub fn get(&self, index: u64) -> Option<T> {
        self.get_inner().get(index)
    }

    /// Updates value in stable memory.
    pub fn append(&mut self, value: T) -> Result<u64> {
        self.mut_inner()
            .append(&value)
            .map_err(|_| Error::OutOfStableMemory)
    }

    /// Number of values in the log.
    pub fn len(&self) -> u64 {
        self.get_inner().len()
    }

    // Returns true, if the Log doesn't contain any values.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Remove all items from the log.
    pub fn clear(&mut self) {
        let inner = self.0.take().expect("inner log is always present");
        let (index_mem, data_mem) = inner.into_memories();
        self.0 = Some(log::Log::new(index_mem, data_mem));
    }

    fn get_inner(&self) -> &log::Log<T, Memory, Memory> {
        self.0.as_ref().expect("inner log is always present")
    }

    fn mut_inner(&mut self) -> &mut log::Log<T, Memory, Memory> {
        self.0.as_mut().expect("inner log is always present")
    }
}

/// Stores key-value data in stable memory.
pub struct StableUnboundedMap<K, V>(unbounded::StableUnboundedMap<Memory, K, V>)
where
    K: BoundedStorable,
    V: SlicedStorable;

impl<K, V> StableUnboundedMap<K, V>
where
    K: BoundedStorable,
    V: SlicedStorable,
{
    /// Create new instance of key-value storage.
    ///
    /// If a memory with the `memory_id` contains data of the map, the map reads it, and the instance
    /// will contain the data from the memory.
    pub fn new(memory_id: MemoryId) -> Self {
        let memory = crate::get_memory_by_id(memory_id);
        Self(unbounded::StableUnboundedMap::new(memory))
    }

    /// Returns a value associated with `key` from stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    pub fn get(&self, key: &K) -> Option<V> {
        self.0.get(key)
    }

    /// Add or replace a value associated with `key` in stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `value.to_bytes().len() <= V::MAX_SIZE`
    pub fn insert(&mut self, key: &K, value: &V) -> Option<V> {
        self.0.insert(key, value)
    }

    /// Remove a value associated with `key` from stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K1::MAX_SIZE`
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.0.remove(key)
    }

    /// List all currently stored key-value pairs.
    pub fn iter(&self) -> unbounded::Iter<'_, Memory, K, V> {
        self.0.iter()
    }

    /// Number of items in the map.
    pub fn len(&self) -> u64 {
        self.0.len()
    }

    // Returns true if there are no values in the map.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Remove all entries from the map.
    pub fn clear(&mut self) {
        self.0.clear()
    }
}

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

    /// Returns if vector is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Removes al the values from the vector
    pub fn clear(&mut self) -> Result<()> {
        self.0 = vec::Vec::<T, Memory>::new(get_memory_by_id(self.1))?;
        Ok(())
    }

    /// Returns the number of elements in the vector
    pub fn len(&self) -> u64 {
        self.0.len()
    }

    /// Sets the value at `index` to `item`
    pub fn set(&mut self, index: u64, item: &T) -> Result<()> {
        self.0.set(index, item);
        Ok(())
    }

    /// Returns the value at `index`
    pub fn get(&self, index: u64) -> Option<T> {
        self.0.get(index)
    }

    /// Appends new value to the vector
    pub fn push(&mut self, item: &T) -> Result<()> {
        self.0.push(item).map_err(Into::into)
    }

    /// Pops the last value from the vector
    pub fn pop(&mut self) -> Option<T> {
        self.0.pop()
    }

    /// Returns iterator over the elements in the vector
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use ic_exports::stable_structures::memory_manager::MemoryId;

    use super::{StableBTreeMap, StableMultimap, StableUnboundedMap};
    use crate::{test_utils, StableVec};

    #[test]
    fn btreemap_works() {
        let mut map = StableBTreeMap::new(MemoryId::new(0));
        assert!(map.is_empty());

        map.insert(0u32, 42u32);
        map.insert(10, 100);
        assert_eq!(map.get(&0), Some(42));
        assert_eq!(map.get(&10), Some(100));

        let mut iter = map.iter();
        assert_eq!(iter.next(), Some((0, 42)));
        assert_eq!(iter.next(), Some((10, 100)));
        assert_eq!(iter.next(), None);

        assert_eq!(map.remove(&10), Some(100));

        assert_eq!(map.len(), 1);
    }

    #[test]
    fn unbounded_map_works() {
        let mut map = StableUnboundedMap::new(MemoryId::new(0));
        assert!(map.is_empty());

        let long_str = test_utils::str_val(50000);
        let medium_str = test_utils::str_val(5000);
        let short_str = test_utils::str_val(50);

        map.insert(&0u32, &long_str);
        map.insert(&3u32, &medium_str);
        map.insert(&5u32, &short_str);
        assert_eq!(map.get(&0).as_ref(), Some(&long_str));
        assert_eq!(map.get(&3).as_ref(), Some(&medium_str));
        assert_eq!(map.get(&5).as_ref(), Some(&short_str));

        let entries: HashMap<_, _> = map.iter().collect();
        let expected = HashMap::from_iter([(0, long_str), (3, medium_str.clone()), (5, short_str)]);
        assert_eq!(entries, expected);

        assert_eq!(map.remove(&3), Some(medium_str));

        assert_eq!(map.len(), 2);
    }

    #[test]
    fn map_works() {
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
