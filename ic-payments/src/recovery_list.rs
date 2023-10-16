use std::borrow::Cow;
use std::cell::RefCell;

use candid::Encode;
use ic_stable_structures::{
    get_memory_by_id, DefaultMemoryManager, DefaultMemoryResourceType,
    DefaultMemoryType, MemoryId, SlicedStorable, StableUnboundedMap, Storable,
    UnboundedMapStructure,
};
use ic_stable_structures::stable_structures::storable::Bound;

use crate::Transfer;

pub trait RecoveryList: Sync + Send {
    fn push(&mut self, transfer: Transfer);
    fn take_all(&mut self) -> Vec<Transfer>;
    fn list(&self) -> Vec<Transfer>;
}

thread_local! {
    static MEMORY_MANAGER: DefaultMemoryManager = DefaultMemoryManager::init(DefaultMemoryResourceType::default());

    static RECOVERY_LIST_STORAGE: RefCell<Option<StableUnboundedMap<TransferKey, TransferValue, TRANSFER_KEY_MAX_SIZE, TRANSFER_KEY_IS_FIXED_SIZE, DefaultMemoryType>>> =
        RefCell::new(None);
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

    const BOUND: Bound = Bound::Bounded { max_size: TRANSFER_KEY_MAX_SIZE as u32, is_fixed_size: TRANSFER_KEY_IS_FIXED_SIZE };
}

const TRANSFER_KEY_MAX_SIZE: usize = 32;
const TRANSFER_KEY_IS_FIXED_SIZE: bool = true;

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

/// The only variable size part of the transfer is memo, which is usually 32 bytes. We use 60 bytes
/// value to account for candid header.
const VALUE_SIZE_OFFSET: usize = 60;

impl SlicedStorable for TransferValue {
    const CHUNK_SIZE: u16 = (std::mem::size_of::<Transfer>() + VALUE_SIZE_OFFSET) as u16;
}

#[derive(Debug)]
pub struct StableRecoveryList<const MEM_ID: u8>;

impl<const MEM_ID: u8> StableRecoveryList<MEM_ID> {
    fn with_storage<R>(
        &self,
        f: impl Fn(&mut StableUnboundedMap<TransferKey, TransferValue, TRANSFER_KEY_MAX_SIZE, TRANSFER_KEY_IS_FIXED_SIZE, DefaultMemoryType>) -> R,
    ) -> R {
        RECOVERY_LIST_STORAGE.with(|v| {
            let mut map = v.borrow_mut();
            let map = map.get_or_insert_with(|| {
                StableUnboundedMap::new(get_memory_by_id(&MEMORY_MANAGER, MemoryId::new(MEM_ID)))
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
            m.insert(&key, &value);
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
