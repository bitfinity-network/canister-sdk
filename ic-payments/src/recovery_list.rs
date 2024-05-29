use std::borrow::Cow;
use std::cell::RefCell;

use candid::Encode;
use ic_stable_structures::stable_structures::storable::Bound;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{
    BTreeMapStructure, IcMemoryManager, MemoryId, StableBTreeMap, Storable, VirtualMemory
};

use crate::Transfer;

pub trait RecoveryList: Sync + Send {
    fn push(&mut self, transfer: Transfer);
    fn take_all(&mut self) -> Vec<Transfer>;
    fn list(&self) -> Vec<Transfer>;
}

thread_local! {
    static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());

    static RECOVERY_LIST_STORAGE: RefCell<Option<StableBTreeMap<TransferKey, TransferValue, VirtualMemory<DefaultMemoryImpl>>>> =
        const { RefCell::new(None) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct TransferKey([u8; 32]);

impl TransferKey {
    fn new(transfer: &Transfer) -> Self {
        Self(transfer.id())
    }
}

impl Storable for TransferKey {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(&self.0[..])
    }

    fn from_bytes(input: Cow<'_, [u8]>) -> Self {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&input);
        Self(bytes)
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: 32,
        is_fixed_size: true,
    };
}

#[derive(Clone)]
struct TransferValue(Transfer);

impl Storable for TransferValue {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let bytes = Encode!(&self.0).expect("serialization of transfer failed");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self(candid::decode_one(&bytes).expect("deserialization of transfer failed"))
    }

    const BOUND: Bound = Bound::Unbounded;
}

#[derive(Debug)]
pub struct StableRecoveryList<const MEM_ID: u8>;

impl<const MEM_ID: u8> StableRecoveryList<MEM_ID> {
    fn with_storage<R>(
        &self,
        f: impl Fn(
            &mut StableBTreeMap<TransferKey, TransferValue, VirtualMemory<DefaultMemoryImpl>>,
        ) -> R,
    ) -> R {
        RECOVERY_LIST_STORAGE.with(|v| {
            let mut map = v.borrow_mut();
            let map = map.get_or_insert_with(|| {
                StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(MemoryId::new(MEM_ID))))
            });
            f(map)
        })
    }
}

impl<const MEM_ID: u8> RecoveryList for StableRecoveryList<MEM_ID> {
    fn push(&mut self, transfer: Transfer) {
        self.with_storage(|m| {
            let key = TransferKey::new(&transfer);
            let value = TransferValue(transfer.clone());

            // It is possible here that a transaction with exact same id is already added to the
            // recovery list. But we don't need to worry about this case, because that would mean
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
