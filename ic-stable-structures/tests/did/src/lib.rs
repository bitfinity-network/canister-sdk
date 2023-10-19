use candid::{CandidType, Decode, Deserialize, Encode};
use ic_stable_structures::stable_structures::storable::Bound;
use ic_stable_structures::{ChunkSize, SlicedStorable, Storable};

pub fn encode(item: &impl CandidType) -> Vec<u8> {
    Encode!(item).expect("failed to encode item to candid")
}

pub fn decode<'a, T: CandidType + Deserialize<'a>>(bytes: &'a [u8]) -> T {
    Decode!(bytes, T).expect("failed to decode item from candid")
}

#[derive(Debug, Default, Clone, Copy, CandidType, Deserialize)]
pub struct BoundedTransaction {
    pub from: u8,
    pub to: u8,
    pub value: u8,
}

impl Storable for BoundedTransaction {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        std::borrow::Cow::Owned([self.from, self.to, self.value].to_vec())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Self {
            from: bytes[0],
            to: bytes[1],
            value: bytes[2],
        }
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: 3,
        is_fixed_size: true,
    };
}

#[derive(Debug, Default, Clone, Copy, CandidType, Deserialize)]
pub struct UnboundedTransaction {
    pub from: u8,
    pub to: u8,
    pub value: u8,
}

impl Storable for UnboundedTransaction {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        encode(self).into()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        decode(&bytes)
    }

    const BOUND: Bound = Bound::Unbounded;
}

impl SlicedStorable for UnboundedTransaction {
    const CHUNK_SIZE: ChunkSize = 8;
}
