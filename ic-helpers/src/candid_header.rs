//! This module provides types and functions that help get and verify the structure of a canister
//! state.

use candid::ser::TypeSerialize;
use candid::{CandidType, Deserialize};
use ic_storage::stable::Versioned;

/// Magic prefix used to signify candid encoded binary.
pub const MAGIC: &[u8] = b"DIDL";

/// Candid header of a versioned state struct.
#[derive(CandidType, Deserialize)]
pub struct CandidHeader {
    /// Version of the state as defined by the `Versioned` trait.
    pub version: u32,

    /// Candid header for the struct, not inluding the magic prefix.
    pub header: Vec<u8>,
}

/// Returns the candid header and version number of the state struct `T`.
pub fn candid_header<T: CandidType + Versioned>() -> CandidHeader {
    let mut type_serializer = TypeSerialize::new();
    type_serializer
        .push_type(&T::ty())
        .expect("should never fail if the state is correct Candid type");
    type_serializer
        .serialize()
        .expect("should never fail if the state is correct Candid type");
    let header = type_serializer.get_result().into();
    let version = T::version();

    CandidHeader { version, header }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candid::{Deserialize, Encode};

    #[test]
    fn test_candid_header() {
        #[derive(CandidType, Deserialize)]
        struct Test {
            field: u32,
        }

        impl Versioned for Test {
            type Previous = ();

            fn upgrade(_: Self::Previous) -> Self {
                Self { field: 0 }
            }
        }

        let header = candid_header::<Test>();

        let ser = Encode!(&Test { field: 0 }).unwrap();
        assert_eq!(header.version, 1);
        assert_eq!(
            header.header[..],
            ser[MAGIC.len()..header.header.len() + MAGIC.len()]
        );
    }
}
