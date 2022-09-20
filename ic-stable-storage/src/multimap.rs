use std::marker::PhantomData;

use candid::{CandidType, Deserialize};

use super::{from_bytes, to_byte_vec, Mem, Memory, Result, StableBTreeMap, VirtualMemory};

/// [`StableMultimap`] stores two keys against a single value, making it possible
/// to fetch all values by the root key, or a single value by specifying both keys.
/// ```
/// use ic_stable_storage::StableMultimap;
/// let mut map = StableMultimap::<_, _, _, 0>::new().unwrap();
/// // Same root key of 1
/// map.insert(1, 1, 1);
/// map.insert(1, 2, 2);
/// // Different root key of 4
/// map.insert(4, 2, 2);
///
/// assert_eq!(map.range(1).unwrap().count(), 2);
/// assert_eq!(map.range(4).unwrap().count(), 1);
/// ```
pub struct StableMultimap<K1, K2, V, const INDEX: u8> {
    _p: PhantomData<(K1, K2, V)>,
    inner: StableBTreeMap<Mem<INDEX>, Vec<u8>, Vec<u8>>,
    k1_len: usize,
}

impl<K1, K2, V, const INDEX: u8> StableMultimap<K1, K2, V, INDEX>
where
    for<'de> K1: CandidType + Deserialize<'de> + Eq + std::hash::Hash + Copy + Default,
    for<'de> K2: CandidType + Deserialize<'de> + Eq + std::hash::Hash + Copy + Default,
    for<'de> V: CandidType + Deserialize<'de> + Copy + Default,
{
    /// Create a new instance of a [`StableMultimap`].
    /// Note that all keys and values has to implement both [`Default`] and [`Copy`] as the keys
    /// and value lenghts are calculated when the map is created.
    pub fn new() -> Result<Self> {
        let key_1_padding = super::calculate_padding::<K1>()?;
        let key_1_len = super::to_byte_vec(&K1::default())?.len() as u32;
        let k1_size = key_1_padding + key_1_len;

        let key_2_padding = super::calculate_padding::<K2>()?;
        let key_2_len = super::to_byte_vec(&K2::default())?.len() as u32;
        let k2_size = key_2_padding + key_2_len;

        let value_padding = super::calculate_padding::<V>()?;
        let value_size = super::to_byte_vec(&V::default())?.len() as u32;

        let inner = crate::MEM.with(|memory| {
            let virt_memory = VirtualMemory::<_, INDEX>::init(memory.clone());
            StableBTreeMap::init(virt_memory, k1_size + k2_size, value_padding + value_size)
        });

        let inst = Self {
            _p: PhantomData,
            k1_len: key_1_len as usize,
            inner,
        };

        Ok(inst)
    }

    /// Insert a new value into the map.
    /// Inserting a value with the same keys as an existing value
    /// will result in the old value being overwritten.
    pub fn insert(&mut self, k1: K1, k2: K2, val: V) -> Result<()> {
        let mut k = to_byte_vec(&k1)?;
        k.extend(to_byte_vec(&k2)?);
        let val = to_byte_vec(&val)?;
        self.inner.insert(k, val)?;
        Ok(())
    }

    /// Get a value for the given keys
    pub fn get(&self, k1: &K1, k2: &K2) -> Option<V> {
        let mut k = to_byte_vec(k1).ok()?;
        k.extend(to_byte_vec(k2).ok()?);

        let bytes = self.inner.get(&k);
        from_bytes(&bytes?).ok()
    }

    /// Remove a specific value and return it
    /// ```
    /// use ic_stable_storage::StableMultimap;
    /// let mut map = StableMultimap::<_, _, _, 0>::new().unwrap();
    /// map.insert(1, 2, 3);
    /// assert_eq!(map.remove(&1, &2).unwrap(), 3);
    /// ```
    pub fn remove(&mut self, k1: &K1, k2: &K2) -> Option<V> {
        let mut k = to_byte_vec(k1).ok()?;
        k.extend(to_byte_vec(k2).ok()?);

        let bytes = self.inner.remove(&k);
        from_bytes(&bytes?).ok()
    }

    /// Remove all values for the partial key
    /// ```
    /// use ic_stable_storage::StableMultimap;
    /// let mut map = StableMultimap::<_, _, _, 0>::new().unwrap();
    /// // Same root key of 1
    /// map.insert(1, 2, 3);
    /// map.insert(1, 3, 4);
    /// // Separate root key, will not be removed.
    /// map.insert(2, 2, 1);
    /// map.remove_partial(&1).unwrap();
    /// assert_eq!(map.len(), 1);
    /// ```
    pub fn remove_partial(&mut self, k1: &K1) -> Result<()> {
        let k = to_byte_vec(k1)?;

        let keys = self
            .inner
            .range(k, None)
            .map(|(k1k2, _)| k1k2)
            .collect::<Vec<_>>();

        for k in keys {
            let _ = self.inner.remove(&k);
        }

        Ok(())
    }

    /// Get a range of key value pairs based on the root key.
    /// ```
    /// use ic_stable_storage::StableMultimap;
    /// let mut map = StableMultimap::<_, _, _, 0>::new().unwrap();
    /// // Same root key of 1
    /// map.insert(1, 2, 3);
    /// map.insert(1, 3, 4);
    /// // Separate root key, will not be included
    /// map.insert(2, 2, 1);
    /// let iter = map.range(1).unwrap();
    /// assert_eq!(iter.count(), 2);
    /// ```
    pub fn range(&self, k1: K1) -> Result<RangeIter<K2, V, Mem<INDEX>>> {
        let bytes = to_byte_vec(&k1)?;
        let inner = self.inner.range(bytes, None);
        let iter = RangeIter {
            key_len: self.k1_len,
            inner,
            _p: PhantomData,
        };
        Ok(iter)
    }

    pub fn len(&self) -> usize {
        self.inner.len() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Range iterator
pub struct RangeIter<'a, K2, V, M: Memory> {
    inner: super::Iter<'a, M, Vec<u8>, Vec<u8>>,
    key_len: usize,
    _p: std::marker::PhantomData<(K2, V)>,
}

// -----------------------------------------------------------------------------
//     - Range Iterator impl -
// -----------------------------------------------------------------------------
impl<'a, K2, V, M: Memory + Clone> Iterator for RangeIter<'a, K2, V, M>
where
    for<'de> K2: CandidType + Deserialize<'de>,
    for<'de> V: CandidType + Deserialize<'de>,
{
    type Item = (K2, V);

    fn next(&mut self) -> Option<(K2, V)> {
        self.inner.next().and_then(|(k1k2, v)| {
            let k2 = from_bytes(&k1k2[self.key_len..]).ok()?;
            let val = from_bytes(&v).ok()?;
            Some((k2, val))
        })
    }
}

/// Iterator
pub struct Iter<'a, K1, K2, V, M: Memory> {
    inner: super::Iter<'a, M, Vec<u8>, Vec<u8>>,
    key_1_len: usize,
    _p: std::marker::PhantomData<(K1, K2, V)>,
}

// -----------------------------------------------------------------------------
//     - Iterator impl -
// -----------------------------------------------------------------------------
impl<'a, K1, K2, V, M: Memory + Clone> Iterator for Iter<'a, K1, K2, V, M>
where
    for<'de> K1: CandidType + Deserialize<'de>,
    for<'de> K2: CandidType + Deserialize<'de>,
    for<'de> V: CandidType + Deserialize<'de>,
{
    type Item = (K1, K2, V);

    fn next(&mut self) -> Option<(K1, K2, V)> {
        self.inner.next().and_then(|(k1k2, v)| {
            let k1 = from_bytes(&k1k2[..self.key_1_len]).ok()?;
            let k2 = from_bytes(&k1k2[self.key_1_len..]).ok()?;
            let val = from_bytes(&v).ok()?;
            Some((k1, k2, val))
        })
    }
}

// -----------------------------------------------------------------------------
//     - Into iterator -
// -----------------------------------------------------------------------------
impl<'a, K1, K2, V, const INDEX: u8> IntoIterator for &'a StableMultimap<K1, K2, V, INDEX>
where
    for<'de> K1: CandidType + Deserialize<'de>,
    for<'de> K2: CandidType + Deserialize<'de>,
    for<'de> V: CandidType + Deserialize<'de>,
{
    type Item = (K1, K2, V);
    type IntoIter = Iter<'a, K1, K2, V, Mem<INDEX>>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            key_1_len: self.k1_len,
            inner: self.inner.iter(),
            _p: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn make_map() -> StableMultimap<[u8; 2], [u8; 3], [u8; 6], 1> {
        let mut mm = StableMultimap::<_, _, _, 1>::new().unwrap();
        let k1 = [1u8, 2];
        let k2 = [11u8, 12, 13];
        let val = [200u8, 200, 200, 100, 100, 123];
        mm.insert(k1, k2, val).unwrap();

        let k1 = [10u8, 20];
        let k2 = [21u8, 22, 23];
        let val = [123, 200u8, 200, 100, 100, 255];
        mm.insert(k1, k2, val).unwrap();

        mm
    }

    #[test]
    fn inserts() {
        let mut mm = StableMultimap::<_, _, _, 1>::new().unwrap();
        for i in 0..10 {
            let k1 = [i; 1];
            let k2 = [i * 10; 2];
            let val = [i; 1];
            mm.insert(k1, k2, val).unwrap();
        }

        assert_eq!(mm.len(), 10);
    }

    #[test]
    fn insert_overwrites() {
        let mut mm = make_map();
        let k1 = [1u8, 2];
        let k2 = [11u8, 12, 13];
        let val = [3u8, 0, 0, 0, 0, 3];
        mm.insert(k1, k2, val).unwrap();

        let ret = mm.get(&k1, &k2).unwrap();

        assert_eq!(val, ret);
    }

    #[test]
    fn get() {
        let mm = make_map();
        let val = mm.get(&[1u8, 2], &[11u8, 12, 13]).unwrap();
        let expected = [200u8, 200, 200, 100, 100, 123];
        assert_eq!(val, expected);
    }

    #[test]
    fn remove() {
        let mut mm = make_map();
        let val = mm.remove(&[1u8, 2], &[11u8, 12, 13]).unwrap();
        let expected = [200u8, 200, 200, 100, 100, 123];
        assert_eq!(val, expected);
        assert_eq!(mm.len(), 1);

        let _ = mm.remove(&[10u8, 20], &[21u8, 22, 23]).unwrap();
        assert!(mm.is_empty());
    }

    #[test]
    fn remove_partial() {
        let mut mm = StableMultimap::<_, _, _, 1>::new().unwrap();
        let k1 = [1u8, 2];
        let k2 = [11u8, 12, 13];
        let val = [200u8, 200, 200, 100, 100, 123];
        mm.insert(k1, k2, val).unwrap();

        let k2 = [21u8, 22, 23];
        let val = [123, 200u8, 200, 100, 100, 255];
        mm.insert(k1, k2, val).unwrap();

        mm.remove_partial(&k1).unwrap();
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
        let k1 = [1u8, 2];
        let mm = make_map();
        let mut iter = mm.range(k1).unwrap();
        assert!(iter.next().is_some());
        assert!(iter.next().is_none());
    }
}
