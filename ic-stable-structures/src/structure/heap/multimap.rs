use std::collections::btree_map::Iter as BTreeMapIter;
use std::{collections::BTreeMap, hash::Hash};

use dfinity_stable_structures::{memory_manager::MemoryId, BoundedStorable};

use crate::structure::MultimapStructure;

/// `HeapMultimap` stores two keys against a single value, making it possible
/// to fetch all values by the root key, or a single value by specifying both keys.
/// The data is stored in the heap memory.

pub struct HeapMultimap<K1, K2, V>(BTreeMap<K1, BTreeMap<K2, V>>)
where
    K1: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    K2: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone;

impl<K1, K2, V> Default for HeapMultimap<K1, K2, V>
where
    K1: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    K2: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<K1, K2, V> HeapMultimap<K1, K2, V>
where
    K1: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    K2: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    /// Create a new instance of a `HeapMultimap`.
    /// All keys and values byte representations should be less then related `..._max_size` arguments.
    pub fn new(_memory_id: MemoryId) -> Self {
        Self(BTreeMap::new())
    }

    /// Get a range of key value pairs based on the root key.
    ///
    /// # Preconditions:
    ///   - `first_key.to_bytes().len() <= K1::MAX_SIZE`
    pub fn range(&self, first_key: &K1) -> HeapMultimapIter<K2, V> {
        match self.0.get(first_key) {
            Some(entry) => HeapMultimapIter(Some(entry.iter())),
            None => HeapMultimapIter(None),
        }
    }

    /// Iterator over all items in map.
    pub fn iter(&self) -> impl Iterator<Item = (K1, K2, V)> + '_ {
        self.0.iter().flat_map(|i1| {
            i1.1.iter()
                .map(|i2| (i1.0.clone(), i2.0.clone(), i2.1.clone()))
        })
    }
}

impl<K1, K2, V> MultimapStructure<K1, K2, V> for HeapMultimap<K1, K2, V>
where
    K1: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    K2: BoundedStorable + Clone + Hash + Eq + PartialEq + Ord,
    V: BoundedStorable + Clone,
{
    fn get(&self, first_key: &K1, second_key: &K2) -> Option<V> {
        self.0
            .get(first_key)
            .and_then(|i| i.get(second_key))
            .cloned()
    }

    fn insert(&mut self, first_key: &K1, second_key: &K2, value: &V) -> Option<V> {
        let entry = self.0.entry(first_key.clone()).or_default();
        entry.insert(second_key.clone(), value.clone())
    }

    fn remove(&mut self, first_key: &K1, second_key: &K2) -> Option<V> {
        self.0
            .get_mut(first_key)
            .and_then(|entry| entry.remove(second_key))
    }

    fn remove_partial(&mut self, first_key: &K1) -> bool {
        self.0.remove(first_key).is_some()
    }

    fn len(&self) -> usize {
        let mut sum = 0;
        for x in self.0.iter() {
            sum += x.1.len();
        }
        sum
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn clear(&mut self) {
        self.0.clear()
    }
}

pub struct HeapMultimapIter<'a, K2, V>(Option<BTreeMapIter<'a, K2, V>>)
where
    K2: BoundedStorable + Clone,
    V: BoundedStorable + Clone;

impl<'a, K2, V> Iterator for HeapMultimapIter<'a, K2, V>
where
    K2: BoundedStorable + Clone,
    V: BoundedStorable + Clone,
{
    type Item = (K2, V);

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.as_mut() {
            Some(item) => {
                let it = item.next();
                it.map(|(k, v)| (k.clone(), v.clone()))
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use dfinity_stable_structures::memory_manager::MemoryId;

    #[test]
    fn multimap_works() {
        let mut map = HeapMultimap::new(MemoryId::new(0));
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

        {
            let mut iter = map.iter();
            assert_eq!(iter.next(), Some((0, 0, 42)));
            assert_eq!(iter.next(), Some((0, 1, 84)));
            assert_eq!(iter.next(), Some((1, 0, 10)));
            assert_eq!(iter.next(), Some((1, 1, 20)));
            assert_eq!(iter.next(), None);
        }

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
}
