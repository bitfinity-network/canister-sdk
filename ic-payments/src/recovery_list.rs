use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use candid::Encode;
use ic_stable_structures::{BoundedStorable, MemoryId, StableBTreeMap, Storable};

use crate::Transfer;

pub trait RecoveryList {
    fn push(&mut self, transfer: Transfer);
    fn take_all(&mut self) -> Vec<Transfer>;
    fn list(&self) -> Vec<Transfer>;
}

impl<T: RecoveryList> RecoveryList for Rc<RefCell<T>> {
    fn push(&mut self, transfer: Transfer) {
        self.borrow_mut().push(transfer)
    }

    fn take_all(&mut self) -> Vec<Transfer> {
        self.borrow_mut().take_all()
    }

    fn list(&self) -> Vec<Transfer> {
        self.borrow_mut().list()
    }
}

thread_local! {
    static RECOVERY_LIST_STORAGE: RefCell<Option<StableBTreeMap<TransferKey, TransferValue>>> =
        RefCell::new(None);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TransferKey([u8; 32]);

impl TransferKey {
    fn new(transfer: &Transfer) -> Self {
        Self(transfer.id())
    }
}

impl Storable for TransferKey {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::from(&self.0[..])
    }

    fn from_bytes(input: Cow<'_, [u8]>) -> Self {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&input);
        Self(bytes)
    }
}

impl BoundedStorable for TransferKey {
    const MAX_SIZE: u32 = 32;
    const IS_FIXED_SIZE: bool = true;
}

struct TransferValue(Transfer);

impl Storable for TransferValue {
    fn to_bytes(&self) -> Cow<[u8]> {
        let bytes = Encode!(&self.0).expect("serialization of transfer failed");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Self(candid::decode_one(&bytes).expect("deserialization of transfer failed"))
    }
}

impl BoundedStorable for TransferValue {
    const MAX_SIZE: u32 = (std::mem::size_of::<Transfer>() + 20) as u32;
    const IS_FIXED_SIZE: bool = true;
}

#[derive(Debug)]
pub struct StableRecoveryList<const MEM_ID: u8>;

impl<const MEM_ID: u8> StableRecoveryList<MEM_ID> {
    fn with_storage<R>(
        &self,
        f: impl Fn(&mut StableBTreeMap<TransferKey, TransferValue>) -> R,
    ) -> R {
        RECOVERY_LIST_STORAGE.with(|v| {
            let mut map = v.borrow_mut();
            if map.is_none() {
                *map = Some(StableBTreeMap::new(MemoryId::new(MEM_ID)));
            }

            f(map.as_mut().unwrap())
        })
    }
}

impl<const MEM_ID: u8> RecoveryList for StableRecoveryList<MEM_ID> {
    fn push(&mut self, transfer: Transfer) {
        self.with_storage(|m| {
            let key = TransferKey::new(&transfer);
            let value = TransferValue(transfer.clone());

            // It is possible here that a transaction with exact same id is already added to the
            // recovery list. But we don't need to worry about this case, becase that would mean
            // that the transactions have same parameters, so deduplication mechanism of ICRC-1
            // tokens would not allow both of such transactions be successful. So we can store only
            // one of them.
            m.insert(key, value);
        })
    }

    fn take_all(&mut self) -> Vec<Transfer> {
        self.with_storage(|m| {
            let list = m.iter().map(|(_, v)| v.0).collect();
            m.clear();
            list
        })
    }

    fn list(&self) -> Vec<Transfer> {
        self.with_storage(|m| m.iter().map(|(_, v)| v.0).collect())
    }
}