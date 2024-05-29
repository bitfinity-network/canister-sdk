use std::ops::RangeBounds;

use dfinity_stable_structures::{btreemap, Memory, Storable};

use crate::structure::BTreeMapStructure;
use crate::IterableSortedMapStructure;

/// Stores key-value data in stable memory.
pub struct StableBTreeMap<K, V, M: Memory>(btreemap::BTreeMap<K, V, M>)
where
    K: Storable + Ord + Clone,
    V: Storable;

impl<K, V, M> StableBTreeMap<K, V, M>
where
    K: Storable + Ord + Clone,
    V: Storable,
    M: Memory,
{
    /// Create new instance of key-value storage.
    pub fn new(memory: M) -> Self {
        Self(btreemap::BTreeMap::init(memory))
    }

    /// Iterate over all currently stored key-value pairs.
    pub fn iter(&self) -> btreemap::Iter<'_, K, V, M> {
        self.0.iter()
    }
}

impl<K, V, M> BTreeMapStructure<K, V> for StableBTreeMap<K, V, M>
where
    K: Storable + Ord + Clone,
    V: Storable,
    M: Memory,
{
    fn get(&self, key: &K) -> Option<V> {
        self.0.get(key)
    }

    fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        self.0.remove(key)
    }

    fn len(&self) -> u64 {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn clear(&mut self) {
        self.0.clear_new();
    }

    fn contains_key(&self, key: &K) -> bool {
        self.0.contains_key(key)
    }

    fn first_key_value(&self) -> Option<(K, V)> {
        self.0.first_key_value()
    }

    fn last_key_value(&self) -> Option<(K, V)> {
        self.0.last_key_value()
    }
}

impl<K, V, M> IterableSortedMapStructure<K, V> for StableBTreeMap<K, V, M>
where
    K: Storable + Ord + Clone,
    V: Storable,
    M: Memory,
{
    type Iterator<'a> = btreemap::Iter<'a, K, V, M> where Self: 'a;

    fn iter(&self) -> Self::Iterator<'_> {
        self.0.iter()
    }

    fn range(&self, key_range: impl RangeBounds<K>) -> Self::Iterator<'_> {
        self.0.range(key_range)
    }

    fn iter_upper_bound(&self, bound: &K) -> Self::Iterator<'_> {
        self.0.iter_upper_bound(bound)
    }
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;

    use dfinity_stable_structures::VectorMemory;

    use super::*;
    use crate::test_utils::str_val;

    #[test]
    fn btreemap_works() {
        let mut map = StableBTreeMap::new(VectorMemory::default());
        assert!(map.is_empty());

        map.insert(0u32, 42u32);
        map.insert(10, 100);
        assert_eq!(map.get(&0), Some(42));
        assert_eq!(map.get(&10), Some(100));

        let mut iter = map.iter();
        assert_eq!(iter.next(), Some((0, 42)));
        assert_eq!(iter.next(), Some((10, 100)));
        assert_eq!(iter.next(), None);

        let mut iter = map.range(1..11);
        assert_eq!(iter.next(), Some((10, 100)));
        assert_eq!(iter.next(), None);

        let mut iter = map.iter_upper_bound(&5);
        assert_eq!(iter.next(), Some((0, 42)));

        assert_eq!(map.remove(&10), Some(100));

        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_last_key_value() {
        let mut map = StableBTreeMap::new(VectorMemory::default());
        assert!(map.is_empty());

        assert!(map.last_key_value().is_none());

        map.insert(0u32, 42u32);
        assert_eq!(map.last_key_value(), Some((0, 42)));

        map.insert(10, 100);
        assert_eq!(map.last_key_value(), Some((10, 100)));

        map.insert(5, 100);
        assert_eq!(map.last_key_value(), Some((10, 100)));

        map.remove(&10);
        assert_eq!(map.last_key_value(), Some((5, 100)));
    }

    #[test]
    fn insert_get_test() {
        let mut map = StableBTreeMap::new(VectorMemory::default());
        assert!(map.is_empty());

        let long_str = str_val(50000);
        let medium_str = str_val(5000);
        let short_str = str_val(50);

        map.insert(0u32, long_str.clone());
        map.insert(3u32, medium_str.clone());
        map.insert(5u32, short_str.clone());

        assert_eq!(map.get(&0).as_ref(), Some(&long_str));
        assert_eq!(map.get(&3).as_ref(), Some(&medium_str));
        assert_eq!(map.get(&5).as_ref(), Some(&short_str));
    }

    #[test]
    fn insert_should_replace_previous_value() {
        let mut map = StableBTreeMap::new(VectorMemory::default());
        assert!(map.is_empty());

        let long_str = str_val(50000);
        let short_str = str_val(50);

        assert!(map.insert(0u32, long_str.clone()).is_none());
        let prev = map.insert(0u32, short_str.clone()).unwrap();

        assert_eq!(&prev, &long_str);
        assert_eq!(map.get(&0).as_ref(), Some(&short_str));
    }

    #[test]
    fn remove_test() {
        let mut map = StableBTreeMap::new(VectorMemory::default());

        let long_str = str_val(50000);
        let medium_str = str_val(5000);
        let short_str = str_val(50);

        map.insert(0u32, long_str.clone());
        map.insert(3u32, medium_str.clone());
        map.insert(5u32, short_str.clone());

        assert_eq!(map.remove(&3), Some(medium_str));

        assert_eq!(map.get(&0).as_ref(), Some(&long_str));
        assert_eq!(map.get(&5).as_ref(), Some(&short_str));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn iter_test() {
        let mut map = StableBTreeMap::new(VectorMemory::default());

        let strs = [str_val(50), str_val(5000), str_val(50000)];

        for i in 0..100u32 {
            map.insert(i, strs[i as usize % strs.len()].clone());
        }

        assert!(map.iter().all(|(k, v)| v == strs[k as usize % strs.len()]))
    }

    #[test]
    fn upper_bound_test() {
        let mut map = StableBTreeMap::new(VectorMemory::default());

        let strs = [str_val(50), str_val(5000), str_val(50000)];

        for i in 0..100u32 {
            map.insert(i, strs[i as usize % strs.len()].clone());
        }

        for i in 1..100u32 {
            let mut iter = map.iter_upper_bound(&i);
            assert_eq!(
                iter.next(),
                Some((i - 1, strs[(i - 1) as usize % strs.len()].clone()))
            );
        }

        let mut iter = map.iter_upper_bound(&0);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn unbounded_map_works() {
        let mut map = StableBTreeMap::new(VectorMemory::default());
        assert!(map.is_empty());

        let long_str = str_val(50000);
        let medium_str = str_val(5000);
        let short_str = str_val(50);

        map.insert(0u32, long_str.clone());
        map.insert(3u32, medium_str.clone());
        map.insert(5u32, short_str.clone());
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
    fn test_first_and_last_key_value() {
        let mut map = StableBTreeMap::new(VectorMemory::default());
        assert!(map.is_empty());

        assert!(map.first_key_value().is_none());
        assert!(map.last_key_value().is_none());

        let str_0 = str_val(50000);
        map.insert(0u32, str_0.clone());

        assert_eq!(map.first_key_value(), Some((0u32, str_0.clone())));
        assert_eq!(map.last_key_value(), Some((0u32, str_0.clone())));

        let str_3 = str_val(5000);
        map.insert(3u32, str_3.clone());

        assert_eq!(map.first_key_value(), Some((0u32, str_0.clone())));
        assert_eq!(map.last_key_value(), Some((3u32, str_3.clone())));

        let str_5 = str_val(50);
        map.insert(5u32, str_5.clone());

        assert_eq!(map.first_key_value(), Some((0u32, str_0.clone())));
        assert_eq!(map.last_key_value(), Some((5u32, str_5.clone())));

        let str_4 = str_val(50);
        map.insert(4u32, str_4.clone());

        assert_eq!(map.first_key_value(), Some((0u32, str_0.clone())));
        assert_eq!(map.last_key_value(), Some((5u32, str_5)));

        map.remove(&5u32);

        assert_eq!(map.first_key_value(), Some((0u32, str_0.clone())));
        assert_eq!(map.last_key_value(), Some((4u32, str_4.clone())));

        let str_4_b = str_val(50);
        map.insert(4u32, str_4_b.clone());

        assert_eq!(map.first_key_value(), Some((0u32, str_0.clone())));
        assert_eq!(map.last_key_value(), Some((4u32, str_4_b)));

        map.remove(&0u32);

        assert_eq!(map.first_key_value(), Some((3u32, str_3)));
        assert_eq!(map.last_key_value(), Some((4u32, str_4)));
    }

    #[test]
    fn btreemap_works_with_composite_keys() {
        let mut map = StableBTreeMap::new(VectorMemory::default());
        assert!(map.is_empty());

        map.insert((0u32, 1u32), 42u32);
        map.insert((10, 0), 0);
        map.insert((10, 6), 60);
        map.insert((10, 3), 30);
        map.insert((11, 5), 55);
        map.insert((10, 5), 50);

        assert_eq!(map.get(&(0, 1)), Some(42));
        assert_eq!(map.get(&(0, 0)), None);
        assert_eq!(map.get(&(10, 0)), Some(0));
        assert_eq!(map.get(&(10, 3)), Some(30));

        assert_eq!(map.get(&(11, 5)), Some(55));
        assert_eq!(map.get(&(10, 5)), Some(50));

        let mut iter = map.iter();
        assert_eq!(iter.next(), Some(((0, 1), 42)));
        assert_eq!(iter.next(), Some(((10, 0), 0)));
        assert_eq!(iter.next(), Some(((10, 3), 30)));
        assert_eq!(iter.next(), Some(((10, 5), 50)));
        assert_eq!(iter.next(), Some(((10, 6), 60)));
        assert_eq!(iter.next(), Some(((11, 5), 55)));
        assert_eq!(iter.next(), None);

        assert_eq!(map.len(), 6);

        let mut iter = map.range((0, 0)..(0, 100));
        assert_eq!(iter.next(), Some(((0, 1), 42)));
        assert_eq!(iter.next(), None);

        let mut iter = map.range((10, 0)..(10, u32::MAX));
        assert_eq!(iter.next(), Some(((10, 0), 0)));
        assert_eq!(iter.next(), Some(((10, 3), 30)));
        assert_eq!(iter.next(), Some(((10, 5), 50)));
        assert_eq!(iter.next(), Some(((10, 6), 60)));
        assert_eq!(iter.next(), None);

        let mut iter = map.range((11, 0)..(11, u32::MAX));
        assert_eq!(iter.next(), Some(((11, 5), 55)));
        assert_eq!(iter.next(), None);

        let mut iter = map.range((10, 0)..(11, u32::MAX));
        assert_eq!(iter.next(), Some(((10, 0), 0)));
        assert_eq!(iter.next(), Some(((10, 3), 30)));
        assert_eq!(iter.next(), Some(((10, 5), 50)));
        assert_eq!(iter.next(), Some(((10, 6), 60)));
        assert_eq!(iter.next(), Some(((11, 5), 55)));
        assert_eq!(iter.next(), None);

        assert_eq!(map.remove(&(10, 3)), Some(30));

        let mut iter = map.range((10, 0)..(10, u32::MAX));
        assert_eq!(iter.next(), Some(((10, 0), 0)));
        assert_eq!(iter.next(), Some(((10, 5), 50)));
        assert_eq!(iter.next(), Some(((10, 6), 60)));
        assert_eq!(iter.next(), None);
    }
}
