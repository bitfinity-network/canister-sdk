use dfinity_stable_structures::BoundedStorable;
use std::collections::btree_map::Iter as BTreeMapIter;
use std::marker::PhantomData;
use std::{collections::BTreeMap, hash::Hash};

use crate::structure::common::SlicedStorable;
use crate::structure::UnboundedMapStructure;

/// Stores key-value data in heap memory.
pub struct HeapUnboundedMap<K, V, M>(BTreeMap<K, V>, PhantomData<M>)
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone;

impl<K, V, M> HeapUnboundedMap<K, V, M>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
{
    /// Create new instance of key-value storage.
    ///
    /// If a memory with the `memory_id` contains data of the map, the map reads it, and the instance
    /// will contain the data from the memory.
    pub fn new(_memory: M) -> Self {
        Self(BTreeMap::new(), Default::default())
    }

    /// List all currently stored key-value pairs.
    pub fn iter(&self) -> HeapUnboundedIter<'_, K, V> {
        HeapUnboundedIter(self.0.iter())
    }
}

impl<K, V, M> UnboundedMapStructure<K, V> for HeapUnboundedMap<K, V, M>
where
    K: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: SlicedStorable + Clone,
{
    fn get(&self, key: &K) -> Option<V> {
        self.0.get(key).cloned()
    }

    fn insert(&mut self, key: &K, value: &V) -> Option<V> {
        self.0.insert(key.clone(), value.clone())
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        self.0.remove(key)
    }

    fn len(&self) -> u64 {
        self.0.len() as u64
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn clear(&mut self) {
        self.0.clear()
    }
}

/// Iterator over values in unbounded map.
/// Constructs a value from chunks on each `next()` call.
pub struct HeapUnboundedIter<'a, K, V>(BTreeMapIter<'a, K, V>)
where
    K: BoundedStorable + Clone,
    V: SlicedStorable + Clone;

impl<'a, K, V> Iterator for HeapUnboundedIter<'a, K, V>
where
    K: BoundedStorable + Clone,
    V: SlicedStorable + Clone,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, v)| (k.clone(), v.clone()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::test_utils;

    #[test]
    fn unbounded_map_works() {
        let mut map = HeapUnboundedMap::new(());
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
        let expected = HashMap::from_iter([
            (0u32, long_str),
            (3u32, medium_str.clone()),
            (5u32, short_str),
        ]);
        assert_eq!(entries, expected);

        assert_eq!(map.remove(&3), Some(medium_str));

        assert_eq!(map.len(), 2);
    }
}
