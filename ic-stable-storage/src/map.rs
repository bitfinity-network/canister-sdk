use std::mem::size_of;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;

use candid::{CandidType, Deserialize};

use super::error::Result;
use super::{from_bytes, Memory, RestrictedMemory, StableBTreeMap, StableMemory, VirtualMemory, PADDING};

type Mem<const INDEX: u8> = VirtualMemory<Rc<RestrictedMemory<StableMemory>>, INDEX>;

/// Inserting the same value twice will simply replace the inner value.
/// ```
/// # use std::collections::HashMap;
/// use ic_stable_storage::StableMap;
/// let hm = HashMap::from([(1u64, 2u8), (3, 4)]);
/// let map = StableMap::<u64, u8, 0>::from(hm);
/// for (key, val) in &map {
/// // ...
/// }
/// ```
pub struct StableMap<K, V, const INDEX: u8> {
    _p: PhantomData<(K, V)>,
    inner: StableBTreeMap<Mem<INDEX>, Vec<u8>, Vec<u8>>,
}

impl<K, V, const INDEX: u8> Default for StableMap<K, V, INDEX> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, const INDEX: u8> StableMap<K, V, INDEX> {
    const MAX_KEY_SIZE: u32 = size_of::<K>() as u32 + PADDING;
    const MAX_VALUE_SIZE: u32 = size_of::<V>() as u32 + PADDING;

    /// Create a new instance of a [`StableMap`].
    pub fn new() -> Self {
        let inner = crate::MEM.with(|memory| {
            let virt_memory = VirtualMemory::<_, INDEX>::init(memory.clone());
            StableBTreeMap::init(virt_memory, Self::MAX_KEY_SIZE, Self::MAX_VALUE_SIZE)
        });

        Self {
            _p: PhantomData,
            inner,
        }
    }

    /// Total count of values.
    /// ```
    /// # use std::collections::HashMap;
    /// # use ic_stable_storage::StableMap;
    /// let hm = HashMap::from([(1u64, 2u64), (3, 4)]);
    /// let mut map = StableMap::<u64, u64, 0>::from(hm);
    /// assert_eq!(map.len(), 2);
    /// ```
    pub fn len(&self) -> u64 {
        self.inner.len()
    }

    /// Check if the `Map` is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<K, V, const INDEX: u8> StableMap<K, V, INDEX>
where
    for<'de> K: CandidType + Deserialize<'de> + Eq + std::hash::Hash + Copy,
    for<'de> V: CandidType + Deserialize<'de> + Copy,
{
    /// Insert a new key/value pair.
    pub fn insert(&mut self, key: K, val: V) -> Result<()> {
        let key_bytes = super::to_byte_vec(&key)?;
        let val_bytes = super::to_byte_vec(&val)?;
        self.inner.insert(key_bytes, val_bytes)?;
        Ok(())
    }

    /// Get a value out of stable storage
    pub fn get(&mut self, key: &K) -> Option<V> {
        let key_bytes = super::to_byte_vec(key).ok()?;
        self.inner.get(&key_bytes).and_then(|val| from_bytes(&val).ok())
    }

    /// Remove a value from the map
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let key_bytes = super::to_byte_vec(key).ok()?;
        let bytes = self.inner.remove(&key_bytes)?;
        from_bytes(&bytes).ok()
    }

    /// Convert the [`Map<K, V>`] into a `HashMap<K, V>`.
    /// This would load and deserialize every value in the `Map` which could be an expensive
    /// operation if there are a lot of values.
    /// ```
    /// # use std::collections::HashMap;
    /// # use ic_stable_storage::StableMap;
    /// let hm = HashMap::from([(1, 1), (2, 2)]);
    /// let mut map = StableMap::<u64, u16, 0>::from(hm.clone());
    /// assert_eq!(map.to_hash_map(), hm);
    /// ```
    pub fn to_hash_map(self) -> HashMap<K, V> {
        self.into_iter().collect()
    }
}

// -----------------------------------------------------------------------------
//     - From hashmap -
// -----------------------------------------------------------------------------
impl<K, V, const INDEX: u8> From<HashMap<K, V>> for StableMap<K, V, INDEX>
where
    for<'de> K: CandidType + Deserialize<'de> + Eq + std::hash::Hash + Copy,
    for<'de> V: CandidType + Deserialize<'de> + Copy,
{
    fn from(hm: HashMap<K, V>) -> Self {
        let mut map = StableMap::new();
        let _ = hm.into_iter().try_for_each(|(k, v)| map.insert(k, v));
        map
    }
}

/// Iterator
pub struct Iter<'a, K, V, M: Memory> {
    inner: super::Iter<'a, M, Vec<u8>, Vec<u8>>,
    _p: std::marker::PhantomData<(K, V)>,
}

// -----------------------------------------------------------------------------
//     - Iterator impl -
// -----------------------------------------------------------------------------
impl<'a, K, V, M: Memory + Clone> Iterator for Iter<'a, K, V, M>
where
    for<'de> K: CandidType + Deserialize<'de>,
    for<'de> V: CandidType + Deserialize<'de>,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<(K, V)> {
        self.inner.next().and_then(|(k, v)| {
            from_bytes(&k)
                .ok()
                .and_then(|k| Some((k, from_bytes(&v).ok()?)))
        })
    }
}

// -----------------------------------------------------------------------------
//     - Into iterator -
// -----------------------------------------------------------------------------
impl<'a, K, V, const INDEX: u8> IntoIterator for &'a StableMap<K, V, INDEX>
where
    for<'de> K: CandidType + Deserialize<'de>,
    for<'de> V: CandidType + Deserialize<'de>,
{
    type Item = (K, V);
    type IntoIter = Iter<'a, K, V, Mem<INDEX>>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            inner: self.inner.iter(),
            _p: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn insert() {
        let mut map = StableMap::<u64, u32, 0>::new();
        let _ = map.insert(1, 3);
        let _ = map.insert(2, 4);

        let expected = HashMap::from([(1, 3), (2, 4)]);
        assert_eq!(map.to_hash_map(), expected);
    }

    #[test]
    fn write_over_existing() {
        let mut map = StableMap::<u64, u32, 0>::new();

        let _ = map.insert(1, 3);
        assert_eq!(map.get(&1), Some(3));

        let _ = map.insert(1, 5);
        assert_eq!(map.get(&1), Some(5));
    }

    #[test]
    fn remove() {
        let hm = HashMap::from([(1, 2), (3, 4), (5, 6)]);
        let mut map = StableMap::<u64, u32, 0>::from(hm);
        assert_eq!(map.remove(&3), Some(4));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn remove_from_empty() {
        let mut map = StableMap::<u64, u32, 0>::new();
        assert_eq!(map.remove(&3), None);
    }

    #[test]
    fn iterator() {
        let hm = HashMap::from([(1, 2), (3, 4)]);
        let map = StableMap::<u64, u8, 0>::from(hm);
        let mut iter = map.into_iter();
        assert_eq!(iter.next(), Some((1, 2)));
        assert_eq!(iter.next(), Some((3, 4)));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn multiple_maps() {
        let map_1 = StableMap::<u64, u8, 0>::from(HashMap::from([(1, 2)]));
        let map_2 = StableMap::<u64, u16, 1>::from(HashMap::from([(2, 3)]));

        let mut iter = map_1.into_iter();
        assert_eq!(iter.next(), Some((1, 2)));
        assert_eq!(iter.next(), None);

        let mut iter = map_2.into_iter();
        assert_eq!(iter.next(), Some((2, 3)));
        assert_eq!(iter.next(), None);
    }
}
