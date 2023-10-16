use candid::{CandidType, Decode, Deserialize, Encode};
use ic_stable_structures::stable_structures::storable::Bound;
use ic_stable_structures::{Storable, SlicedStorable, ChunkSize};

pub fn encode(item: &impl CandidType) -> Vec<u8> {
    Encode!(item).expect("failed to encode item to candid")
}

pub fn decode<'a, T: CandidType + Deserialize<'a>>(bytes: &'a [u8]) -> T {
    Decode!(bytes, T).expect("failed to decode item from candid")
}

#[derive(Debug, Default, Clone, Copy, CandidType, Deserialize)]
pub struct Transaction {
    pub from: u8,
    pub to: u8,
    pub value: u8,
}

impl Storable for Transaction {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        encode(self).into()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        decode(&bytes)
    }

    const BOUND: Bound = Bound::Unbounded;
}

impl SlicedStorable for Transaction {
    const CHUNK_SIZE: ChunkSize = 8;
}