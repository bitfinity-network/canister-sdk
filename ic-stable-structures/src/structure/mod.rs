use std::ops::RangeBounds;

use crate::Result;

mod cache;
mod common;
mod heap;
mod stable_storage;

pub use cache::*;
pub use common::*;
pub use heap::*;
pub use stable_storage::*;

pub trait BTreeMapStructure<K, V> {
    /// Return value associated with `key` from stable memory.
    fn get(&self, key: &K) -> Option<V>;

    /// Add or replace value associated with `key` in stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    ///   - `value.to_bytes().len() <= V::MAX_SIZE`
    fn insert(&mut self, key: K, value: V) -> Option<V>;

    /// Remove value associated with `key` from stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    fn remove(&mut self, key: &K) -> Option<V>;

    /// True if contains the key.
    fn contains_key(&self, key: &K) -> bool;

    /// Returns the last key-value pair in the map.
    fn last_key_value(&self) -> Option<(K, V)>;

    /// Count of items in the map.
    fn len(&self) -> u64;

    /// Is the map empty.
    fn is_empty(&self) -> bool;

    /// Remove all entries from the map.
    fn clear(&mut self);
}

/// Map that supports ordered iterator
pub trait IterableSortedMapStructure<K, V> {
    /// Map iterator type
    type Iterator<'a>: Iterator<Item = (K, V)>
    where
        Self: 'a;

    /// Returns iterator over the whole collection
    fn iter(&self) -> Self::Iterator<'_>;

    /// Returns an iterator over the entries in the map where keys
    /// belong to the specified range.
    fn range(&self, key_range: impl RangeBounds<K>) -> Self::Iterator<'_>;

    /// Returns an iterator pointing to the first element below the given bound.
    /// Returns an empty iterator if there are no keys below the given bound.
    fn iter_upper_bound(&self, bound: &K) -> Self::Iterator<'_>;
}

pub trait CellStructure<T> {
    /// Returns reference to value stored in stable memory.
    fn get(&self) -> &T;

    /// Updates value in stable memory.
    fn set(&mut self, value: T) -> Result<()>;
}

pub trait LogStructure<T> {
    /// Returns reference to value stored in stable memory.
    fn get(&self, index: u64) -> Option<T>;

    /// Updates value in stable memory.
    fn append(&mut self, value: T) -> Result<u64>;

    /// Number of values in the log.
    fn len(&self) -> u64;

    // Returns true, if the Log doesn't contain any values.
    fn is_empty(&self) -> bool;

    /// Remove all items from the log.
    fn clear(&mut self);
}

pub trait MultimapStructure<K1, K2, V> {
    /// iterator over the whole map
    type Iterator<'a>: Iterator<Item = (K1, K2, V)>
    where
        Self: 'a;

    /// Iterator over the values that correspond to some `K1` key
    type RangeIterator<'a>: Iterator<Item = (K2, V)>
    where
        Self: 'a;

    /// Get a value for the given keys.
    /// If byte representation length of any key exceeds max size, `None` will be returned.
    fn get(&self, first_key: &K1, second_key: &K2) -> Option<V>;

    /// Insert a new value into the map.
    /// Inserting a value with the same keys as an existing value
    /// will result in the old value being overwritten.
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `second_key.to_bytes().len() <= K2::MAX_SIZE`
    ///   - `value.to_bytes().len() <= V::MAX_SIZE`
    fn insert(&mut self, first_key: &K1, second_key: &K2, value: &V) -> Option<V>;

    /// Remove a specific value and return it.
    ///
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `second_key.to_bytes().len() <= K2::MAX_SIZE`
    fn remove(&mut self, first_key: &K1, second_key: &K2) -> Option<V>;

    /// Remove all values for the partial key
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    fn remove_partial(&mut self, first_key: &K1) -> bool;

    /// Items count.
    fn len(&self) -> usize;

    /// Is map empty.
    fn is_empty(&self) -> bool;

    /// Iterator over all the entries that korrespond to the `first_key`
    fn range(&self, first_key: &K1) -> Self::RangeIterator<'_>;

    /// Iterator over all items in the map.
    fn iter(&self) -> Self::Iterator<'_>;

    /// Remove all entries from the map.
    fn clear(&mut self);
}

pub trait UnboundedMapStructure<K, V> {
    /// Returns a value associated with `key` from heap memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K::MAX_SIZE`
    fn get(&self, key: &K) -> Option<V>;

    /// Returns the first key in the map.
    fn first_key(&self) -> Option<K>;

    /// Returns the first key-value pair in the map.
    fn first_key_value(&self) -> Option<(K, V)>;

    /// Returns the last key in the map.
    fn last_key(&self) -> Option<K>;

    /// Returns the last key-value pair in the map.
    fn last_key_value(&self) -> Option<(K, V)>;

    /// Add or replace a value associated with `key` in stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K1::MAX_SIZE`
    ///   - `value.to_bytes().len() <= V::MAX_SIZE`
    fn insert(&mut self, key: &K, value: &V) -> Option<V>;

    /// Remove a value associated with `key` from stable memory.
    ///
    /// # Preconditions:
    ///   - `key.to_bytes().len() <= K1::MAX_SIZE`
    fn remove(&mut self, key: &K) -> Option<V>;

    /// Number of items in the map.
    fn len(&self) -> u64;

    /// Retuns total number of chunks, used to store all the items.
    fn total_chunks_number(&self) -> u64;

    // Returns true if there are no values in the map.
    fn is_empty(&self) -> bool;

    /// Remove all entries from the map.
    fn clear(&mut self);
}

pub trait VecStructure<T> {
    /// Returns if vector is empty
    fn is_empty(&self) -> bool;

    /// Removes al the values from the vector
    fn clear(&mut self) -> Result<()>;

    /// Returns the number of elements in the vector
    fn len(&self) -> u64;

    /// Sets the value at `index` to `item`
    /// WARN: this panics if index out of range
    fn set(&mut self, index: u64, item: &T) -> Result<()>;

    /// Returns the value at `index`
    fn get(&self, index: u64) -> Option<T>;

    /// Appends new value to the vector
    fn push(&mut self, item: &T) -> Result<()>;

    /// Pops the last value from the vector
    fn pop(&mut self) -> Option<T>;
}
