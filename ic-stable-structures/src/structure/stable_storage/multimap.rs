use dfinity_stable_structures::{btreemap, Memory, StableBTreeMap, Storable};

use crate::structure::MultimapStructure;
use crate::Bounded;

/// `StableMultimap` stores two keys against a single value, making it possible
/// to fetch all values by the root key, or a single value by specifying both keys.
pub struct StableMultimap<K1, K2, V, M>(StableBTreeMap<(K1, K2), V, M>)
where
    K1: Storable + Ord + Clone,
    K2: Storable + Ord + Clone + Bounded<K2>,
    V: Storable,
    M: Memory;

impl<K1, K2, V, M> StableMultimap<K1, K2, V, M>
where
    K1: Storable + Ord + Clone,
    K2: Storable + Ord + Clone + Bounded<K2>,
    V: Storable,
    M: Memory,
{
    /// Create a new instance of a `StableMultimap`.
    pub fn new(memory: M) -> Self {
        Self(StableBTreeMap::init(memory))
    }

    /// Returns upper bound iterator for the given pair of keys.
    pub fn iter_upper_bound(&self, key: &(K1, K2)) -> StableMultimapIter<'_, K1, K2, V, M> {
        StableMultimapIter::new(self.0.iter_upper_bound(&key))
    }
}

impl<K1, K2, V, M> MultimapStructure<K1, K2, V> for StableMultimap<K1, K2, V, M>
where
    K1: Storable + Ord + Clone,
    K2: Storable + Ord + Clone + Bounded<K2>,
    V: Storable,
    M: Memory,
{
    type Iterator<'a> = StableMultimapIter<'a, K1, K2, V, M> where Self: 'a;

    type RangeIterator<'a> = StableMultimapRangeIter<'a, K1, K2, V, M> where Self: 'a;

    fn insert(&mut self, first_key: &K1, second_key: &K2, value: V) -> Option<V> {
        self.0
            .insert((first_key.clone(), second_key.clone()), value)
    }

    fn get(&self, first_key: &K1, second_key: &K2) -> Option<V> {
        self.0.get(&(first_key.clone(), second_key.clone()))
    }

    fn remove(&mut self, first_key: &K1, second_key: &K2) -> Option<V> {
        self.0.remove(&(first_key.clone(), second_key.clone()))
    }

    fn remove_partial(&mut self, first_key: &K1) -> bool {
        let keys: Vec<_> = self
            .0
            .range((first_key.clone(), K2::MIN)..=(first_key.clone(), K2::MAX))
            .map(|(keys, _)| keys)
            .collect();

        let mut found = false;
        for k in keys {
            found = self.0.remove(&k).is_some() || found;
        }
        found
    }

    fn len(&self) -> usize {
        self.0.len() as usize
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn clear(&mut self) {
        self.0.clear_new();
    }

    fn range(&self, first_key: &K1) -> Self::RangeIterator<'_> {
        let inner = self
            .0
            .range((first_key.clone(), K2::MIN)..=(first_key.clone(), K2::MAX));
        StableMultimapRangeIter::new(inner)
    }

    fn iter(&self) -> Self::Iterator<'_> {
        StableMultimapIter::new(self.0.iter())
    }
}

/// Range iterator
pub struct StableMultimapRangeIter<'a, K1, K2, V, M>
where
    K1: Storable + Ord + Clone,
    K2: Storable + Ord + Clone,
    V: Storable,
    M: Memory,
{
    inner: btreemap::Iter<'a, (K1, K2), V, M>,
}

impl<'a, K1, K2, V, M> StableMultimapRangeIter<'a, K1, K2, V, M>
where
    K1: Storable + Ord + Clone,
    K2: Storable + Ord + Clone,
    V: Storable,
    M: Memory,
{
    fn new(inner: btreemap::Iter<'a, (K1, K2), V, M>) -> Self {
        Self { inner }
    }
}

// -----------------------------------------------------------------------------
//     - Range Iterator impl -
// -----------------------------------------------------------------------------
impl<'a, K1, K2, V, M> Iterator for StableMultimapRangeIter<'a, K1, K2, V, M>
where
    K1: Storable + Ord + Clone,
    K2: Storable + Ord + Clone,
    V: Storable,
    M: Memory,
{
    type Item = (K2, V);

    fn next(&mut self) -> Option<(K2, V)> {
        self.inner.next().map(|(keys, v)| (keys.1, v))
    }
}

pub struct StableMultimapIter<'a, K1, K2, V, M>(btreemap::Iter<'a, (K1, K2), V, M>)
where
    K1: Storable + Ord + Clone,
    K2: Storable + Ord + Clone,
    V: Storable,
    M: Memory;

impl<'a, K1, K2, V, M> StableMultimapIter<'a, K1, K2, V, M>
where
    K1: Storable + Ord + Clone,
    K2: Storable + Ord + Clone,
    V: Storable,
    M: Memory,
{
    fn new(inner: btreemap::Iter<'a, (K1, K2), V, M>) -> Self {
        Self(inner)
    }
}

impl<'a, K1, K2, V, M> Iterator for StableMultimapIter<'a, K1, K2, V, M>
where
    K1: Storable + Ord + Clone,
    K2: Storable + Ord + Clone,
    V: Storable,
    M: Memory,
{
    type Item = (K1, K2, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(keys, val)| {
            let k1 = keys.0;
            let k2 = keys.1;
            (k1, k2, val)
        })
    }
}

impl<'a, K1, K2, V, M> IntoIterator for &'a StableMultimap<K1, K2, V, M>
where
    K1: Storable + Ord + Clone,
    K2: Storable + Ord + Clone + Bounded<K2>,
    V: Storable,
    M: Memory,
{
    type Item = (K1, K2, V);

    type IntoIter = StableMultimapIter<'a, K1, K2, V, M>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod test {

    use dfinity_stable_structures::VectorMemory;

    use super::*;
    use crate::test_utils::Array;

    fn make_map() -> StableMultimap<Array<2>, Array<3>, Array<6>, VectorMemory> {
        let mut mm = StableMultimap::new(VectorMemory::default());
        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = Array([200u8, 200, 200, 100, 100, 123]);
        mm.insert(&k1, &k2, val);

        let k1 = Array([10u8, 20]);
        let k2 = Array([21u8, 22, 23]);
        let val = Array([123, 200u8, 200, 100, 100, 255]);
        mm.insert(&k1, &k2, val);

        mm
    }

    #[test]
    fn inserts() {
        let mut mm: StableMultimap<Array<1>, Array<2>, Array<1>, _> =
            StableMultimap::new(VectorMemory::default());
        for i in 0..10 {
            let k1 = Array([i; 1]);
            let k2 = Array([i * 10; 2]);
            let val = Array([i; 1]);
            mm.insert(&k1, &k2, val);
        }

        assert_eq!(mm.len(), 10);
    }

    #[test]
    fn insert_should_replace_old_value() {
        let mut mm = make_map();

        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = Array([255u8, 255, 255, 255, 255, 255]);

        let prev_val = Array([200u8, 200, 200, 100, 100, 123]);
        let replaced_val = mm.insert(&k1, &k2, val).unwrap();

        assert_eq!(prev_val, replaced_val);
        assert_eq!(mm.get(&k1, &k2), Some(val));
    }

    #[test]
    fn get() {
        let mm = make_map();
        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = mm.get(&k1, &k2).unwrap();

        let expected = Array([200u8, 200, 200, 100, 100, 123]);
        assert_eq!(val, expected);
    }

    #[test]
    fn remove() {
        let mut mm = make_map();
        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = mm.remove(&k1, &k2).unwrap();

        let expected = Array([200u8, 200, 200, 100, 100, 123]);
        assert_eq!(val, expected);
        assert_eq!(mm.len(), 1);

        let k1 = Array([10u8, 20]);
        let k2 = Array([21u8, 22, 23]);
        mm.remove(&k1, &k2).unwrap();
        assert!(mm.is_empty());
    }

    #[test]
    fn remove_partial() {
        let mut mm = StableMultimap::new(VectorMemory::default());
        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = Array([200u8, 200, 200, 100, 100, 123]);
        mm.insert(&k1, &k2, val);

        let k2 = Array([21u8, 22, 23]);
        let val = Array([123, 200u8, 200, 100, 100, 255]);
        mm.insert(&k1, &k2, val);

        assert!(mm.remove_partial(&k1));
        assert!(!mm.remove_partial(&k1));
        assert!(mm.is_empty());
    }

    #[test]
    fn clear() {
        let mut mm = StableMultimap::new(VectorMemory::default());
        let k1 = Array([1u8, 2]);
        let k2 = Array([11u8, 12, 13]);
        let val = Array([200u8, 200, 200, 100, 100, 123]);
        mm.insert(&k1, &k2, val);

        let k2 = Array([21u8, 22, 23]);
        let val = Array([123, 200u8, 200, 100, 100, 255]);
        mm.insert(&k1, &k2, val);
        let k1 = Array([21u8, 22]);
        mm.insert(&k1, &k2, val);

        mm.clear();
        assert!(mm.is_empty());
    }

    #[test]
    fn iter() {
        let mm = make_map();
        let mut iter = mm.into_iter();
        assert!(iter.next().is_some());
        assert!(iter.next().is_some());
        assert!(iter.next().is_none());
    }

    #[test]
    fn range_iter() {
        let k1 = Array([1u8, 2]);
        let mm = make_map();
        let mut iter = mm.range(&k1);
        assert!(iter.next().is_some());
        assert!(iter.next().is_none());
    }

    #[test]
    fn iter_upper_bound() {
        let mm = make_map();
        assert_eq!(
            mm.iter_upper_bound(&(Array([0, 0]), Array([0, 0, 0])))
                .next(),
            None
        );
        assert_eq!(
            mm.iter_upper_bound(&(Array([1, 2]), Array([0, 0, 0])))
                .next(),
            None
        );
        assert_eq!(
            mm.iter_upper_bound(&(Array([1, 2]), Array([11, 12, 13])))
                .next(),
            None
        );
        assert_eq!(
            mm.iter_upper_bound(&(Array([1, 2]), Array([15, 16, 17])))
                .next(),
            Some((
                Array([1, 2]),
                Array([11, 12, 13]),
                Array([200, 200, 200, 100, 100, 123])
            ))
        );
        assert_eq!(
            mm.iter_upper_bound(&(Array([10, 20]), Array([15, 16, 17])))
                .next(),
            Some((
                Array([1, 2]),
                Array([11, 12, 13]),
                Array([200, 200, 200, 100, 100, 123])
            ))
        );
        assert_eq!(
            mm.iter_upper_bound(&(Array([10, 20]), Array([21, 22, 23])))
                .next(),
            Some((
                Array([1, 2]),
                Array([11, 12, 13]),
                Array([200, 200, 200, 100, 100, 123])
            ))
        );
        assert_eq!(
            mm.iter_upper_bound(&(Array([10, 20]), Array([21, 22, 25])))
                .next(),
            Some((
                Array([10, 20]),
                Array([21, 22, 23]),
                Array([123, 200, 200, 100, 100, 255])
            ))
        );
        assert_eq!(
            mm.iter_upper_bound(&(Array([10, 21]), Array([0, 0, 0])))
                .next(),
            Some((
                Array([10, 20]),
                Array([21, 22, 23]),
                Array([123, 200, 200, 100, 100, 255])
            ))
        );
    }

    #[test]
    fn multimap_works() {
        let mut map = StableMultimap::new(VectorMemory::default());
        assert!(map.is_empty());

        map.insert(&0u32, &0u32, 42u32);
        map.insert(&0u32, &1u32, 84u32);

        map.insert(&1u32, &0u32, 10u32);
        map.insert(&1u32, &1u32, 20u32);

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
