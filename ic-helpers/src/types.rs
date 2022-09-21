use ic_exports::ic_cdk::export::candid::{CandidType, Deserialize};

#[derive(CandidType, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pubkey(Vec<u8>);

impl std::default::Default for Pubkey {
    fn default() -> Self {
        Pubkey::empty()
    }
}

impl Pubkey {
    pub fn new(pubkey: Vec<u8>) -> Self {
        Self(pubkey)
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
