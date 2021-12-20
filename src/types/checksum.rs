use candid::CandidType;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// An alias to define version type
pub type Version = usize;

/// Represents a hashed checksum to compare versions of wasm modules
#[derive(CandidType, Clone, Serialize, Deserialize, Default, Eq)]
pub struct Checksum {
    pub version: Version,
    hash: Vec<u8>,
}

impl Checksum {
    pub fn upgrade(&mut self, other: Self) {
        if *self != other {
            let (version, _) = self.version.overflowing_add(1);
            self.version = version;
            self.hash = other.hash;
        }
    }
}

impl From<&[u8]> for Checksum {
    fn from(wasm_module: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(wasm_module);
        Self {
            hash: hasher.finalize().as_slice().into(),
            version: 0,
        }
    }
}

impl PartialEq for Checksum {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl ToString for Checksum {
    fn to_string(&self) -> String {
        hex::encode(self.hash.as_slice())
    }
}
