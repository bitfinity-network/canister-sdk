use candid::{CandidType, Deserialize, Principal};
use ic_ic00_types::Payload;

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

#[derive(CandidType, Deserialize, Debug)]
pub struct GetECDSAPublicKeyArgs {
    pub canister_id: Option<Principal>,
    pub derivation_path: Vec<Vec<u8>>,
    pub key_id: String,
}

/// This is a structure that is only used for `get_ecdsa_pubkey` call in
/// management canister. The only problem that it is only used by dfx in
/// November build of ic, but as a dependency we use the more recent one,
/// hence, to be able to deserialized the result of `get_ecdsa_pubkey`
/// we need something that represents its response.
#[derive(CandidType, Deserialize, Debug)]
pub struct GetECDSAPublicKeyResponse {
    pub public_key: Vec<u8>,
    pub chain_code: Vec<u8>,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct SignWithECDSAReply {
    pub signature: Vec<u8>,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct SignWithECDSAArgs {
    pub message_hash: Vec<u8>,
    pub derivation_path: Vec<Vec<u8>>,
    pub key_id: String,
}

impl Payload<'_> for SignWithECDSAArgs {}
