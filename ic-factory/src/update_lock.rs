use crate::error::FactoryError;
use ic_exports::ic_cdk::export::candid::{CandidType, Deserialize};
use std::cell::RefCell;
use std::rc::Rc;

/// A guard to prevent factory state changes while an async operation is in process.
///
/// The guarded structure stores the original lock object as one of its fields. To set the lock
/// [`lock`] method is called, returning a clone of this object in the locked state. Any consequent
/// calls to the `lock` method will return an error until the first lock is dropped.
///
/// We need to use this lock type instead of relying on `RefCell` borrowed `Ref` as a lock because
/// it is possible in IC environment to get the canister to a state when the `Ref` object is lost
/// without dropping it, which makes the state locked forever. The `UpdateLock` allows to fix such
/// state by calling the [`unlock`] method.
#[derive(Debug, Default)]
pub struct UpdateLock {
    is_locked: Rc<RefCell<bool>>,
}

impl UpdateLock {
    /// Set the lock into the locked state, returning a copy of the lock. The lock will be locked
    /// until the returned object is dropped.
    ///
    /// # Errors
    ///
    /// If the lock is already in the locked state, calling `lock` will return
    /// `Err(FactoryError::StateLocked)` error.
    pub fn lock(&self) -> Result<Self, FactoryError> {
        if *self.is_locked.borrow() {
            return Err(FactoryError::StateLocked);
        }

        self.is_locked.replace(true);
        Ok(Self {
            is_locked: self.is_locked.clone(),
        })
    }

    /// Returns if the lock is locked.
    pub fn is_locked(&self) -> bool {
        *self.is_locked.borrow()
    }

    /// Resets the state of the lock to be unlocked.
    ///
    /// This method is supposed to be used only to fix a broken state, which can happen in case a
    /// panic occurs after `await` in an async update method, while the lock is not released. That
    /// is the reason why this method is `pub(crate)` - to limit the places where it can be used.
    pub(crate) fn unlock(&self) {
        self.is_locked.replace(false);
    }
}

impl Drop for UpdateLock {
    fn drop(&mut self) {
        self.is_locked.replace(false);
    }
}

impl PartialEq for UpdateLock {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.is_locked, &other.is_locked)
    }
}

impl CandidType for UpdateLock {
    fn _ty() -> candid::types::Type {
        candid::types::Type::Bool
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: candid::types::Serializer,
    {
        (*self.is_locked.borrow()).idl_serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for UpdateLock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let val = bool::deserialize(deserializer)?;
        Ok(Self {
            is_locked: Rc::new(RefCell::new(val)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candid::{Decode, Encode};

    #[test]
    fn unlock_on_drop() {
        let original = UpdateLock::default();
        assert!(!original.is_locked());

        let lock = original.lock().unwrap();
        assert!(original.is_locked());
        assert!(lock.is_locked());

        drop(lock);
        assert!(!original.is_locked());
    }

    #[test]
    fn lock_serialization() {
        let original = UpdateLock::default();
        let lock = original.lock().unwrap();

        let encoded = Encode!(&lock).unwrap();
        let decoded = Decode!(&encoded, UpdateLock).unwrap();

        assert!(decoded.is_locked(), "not locked after decoding");
    }

    #[test]
    fn double_lock() {
        let original = UpdateLock::default();
        let _lock = original.lock().unwrap();

        assert!(matches!(original.lock(), Err(FactoryError::StateLocked)));
    }

    #[test]
    fn admin_unlocking() {
        let original = UpdateLock::default();
        let _lock = original.lock().unwrap();
        original.unlock();

        assert!(!original.is_locked());
    }
}
