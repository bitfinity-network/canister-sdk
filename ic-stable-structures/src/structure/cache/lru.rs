use std::convert::Infallible;
use std::hash::Hash;

use parking_lot::Mutex;
use schnellru::{ByLength, LruMap};

/// A wrapper around `LruCache`. This struct is thread safe, doesn't return any references to any
/// elements inside.
pub struct SyncLruCache<K, V> {
    inner: Mutex<LruMap<K, V>>,
}

impl<K, V> SyncLruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Creats a new `LRU` cache that holds at most `cap` items.
    pub fn new(cap: u32) -> Self {
        Self {
            // Creating an inner LruMap with a fixed hasher
            inner: Mutex::new(LruMap::<K, V>::with_seed(ByLength::new(cap), [0, 1, 3, 4])),
        }
    }

    /// Returns the number of key-value pairs that are currently in the the cache.
    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    /// Returns true if the cache is empty and false otherwise.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }

    /// Return the value of they key in the cache otherwise computes the value and inserts it into
    /// the cache. If the key is already in the cache, they gets gets moved to the head of
    /// the LRU list.
    pub fn get_or_insert_with<F>(&self, key: &K, f: F) -> Option<V>
    where
        V: Clone,
        F: FnOnce(&K) -> Option<V>,
    {
        Result::<_, Infallible>::unwrap(self.get_or_try_insert_with(key, |k| Ok(f(k))))
    }

    /// Returns the value of they key in the cache if present, otherwise
    /// computes the value using the provided closure.
    ///
    /// If the key is already in the cache, it gets moved to the head of the LRU
    /// list.
    ///
    /// If the provided closure fails, the error is returned and the cache is
    /// not updated.
    pub fn get_or_try_insert_with<F, E>(&self, key: &K, f: F) -> Result<Option<V>, E>
    where
        V: Clone,
        F: FnOnce(&K) -> Result<Option<V>, E>,
    {
        if let Some(result) = self.get(key) {
            return Ok(Some(result));
        }
        let val = f(key)?;
        if let Some(val) = val.as_ref() {
            let val_clone = val.clone();
            self.inner.lock().insert(key.clone(), val_clone);
        }
        Ok(val)
    }

    /// Puts a key-value pair into cache. If the key already exists in the cache,
    /// then it updates the key's value.
    pub fn insert(&self, key: K, value: V) {
        self.inner.lock().insert(key, value);
    }

    /// Returns whether the key is in the cache
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner.lock().get(key).is_some()
    }

    /// Returns the value of the key in the cache or None if it is not present in the cache.
    /// Moves the key to the head of the LRU list if it exists.
    pub fn get(&self, key: &K) -> Option<V> {
        self.inner.lock().get(key).cloned()
    }

    /// Removes an element from the cache.
    pub fn remove(&self, key: &K) -> Option<V> {
        self.inner.lock().remove(key)
    }

    /// Puts a key-value pair into cache. If the key already exists in the cache,
    /// then it updates the key's value.
    pub fn clear(&self) {
        self.inner.lock().clear()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_cache() {
        let cache = SyncLruCache::<u64, Vec<u64>>::new(100.try_into().unwrap());

        assert_eq!(cache.get(&0u64), None);
        assert_eq!(
            cache.get_or_insert_with(&123u64, |key| Some(vec![*key, 123])),
            Some(vec![123u64, 123])
        );
        assert_eq!(cache.get(&123u64), Some(vec![123u64, 123]));
        assert!(cache.contains_key(&123u64));
        assert_eq!(cache.get_or_insert_with(&127u64, |_| None), None);
        assert_eq!(cache.get(&0u64), None);
        assert!(!cache.contains_key(&0u64));
    }
}
