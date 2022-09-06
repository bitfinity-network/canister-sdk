//! This module provides types and functions that help get and verify the structure of a canister
//! state.

use candid::ser::TypeSerialize;
use candid::{CandidType, Deserialize};
use ic_storage::stable::Versioned;

/// Magic prefix used to signify candid encoded binary.
pub const MAGIC: &[u8] = b"DIDL";

/// Candid header of a versioned state struct.
///
/// When candid serializes structures, the resulting binary consists of three parts:
/// 1. The [`MAGIC`] prefix, signifying that the binary is acutally a candid serialized file.
/// 2. The header containing definition of the serialized type. This header includes field order,
///    names and types.
/// 3. Actual values of the fields.
///
/// This sturcture represents the second part of the candid serialized structure - the header,
/// aloong with the version of the type as defined by the `Versioned` trait implementation.
///
/// This header can be used to transfer information about the type between canisters or to verify
/// that the type used by a canister is what the consumr expects.
#[derive(Debug, CandidType, Deserialize, Clone, PartialEq, Eq)]
pub struct CandidHeader {
    /// Version of the state as defined by the `Versioned` trait.
    pub version: u32,

    /// Candid header for the struct, not inluding the [`MAGIC`] prefix.
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

#[derive(Debug, CandidType, Deserialize)]
pub enum TypeCheckResult {
    Ok {
        remote_version: u32,
        current_version: u32,
    },
    Error {
        remote_version: u32,
        current_version: u32,
        error_message: String,
    },
}

impl TypeCheckResult {
    pub fn is_err(&self) -> bool {
        matches!(self, TypeCheckResult::Error { .. })
    }
}

pub fn validate_header<T: CandidType + Versioned>(remote_header: &CandidHeader) -> TypeCheckResult {
    let current_version = T::version();

    match get_historic_header::<T>(remote_header.version) {
        Some(historic_header) if historic_header == remote_header.header => TypeCheckResult::Ok {
            remote_version: remote_header.version,
            current_version,
        },
        Some(historic_header) => TypeCheckResult::Error {
            remote_version: remote_header.version,
            current_version,
            error_message: generate_state_error(remote_header, &historic_header),
        },
        None => TypeCheckResult::Error {
            remote_version: remote_header.version,
            current_version,
            error_message: "The remote type is not a historic version of the current type".into(),
        },
    }
}

pub fn get_historic_header<T: Versioned + CandidType>(version: u32) -> Option<Vec<u8>> {
    if T::version() == 0 {
        None
    } else if T::version() == version {
        Some(candid_header::<T>().header)
    } else {
        get_historic_header::<T::Previous>(version)
    }
}

fn generate_state_error(canister_state: &CandidHeader, historic_state_header: &[u8]) -> String {
    let canister_type = match get_type_definition(&canister_state.header) {
        Ok(type_definition) => type_definition,
        Err(e) => return e,
    };

    let crate_type = match get_type_definition(historic_state_header) {
        Ok(type_definition) => type_definition,
        Err(e) => return e,
    };

    format!("The canister state structure differs from the expected state structure of the same version.

Canister state:
{canister_type}


Expected state type of the same state version:
{crate_type}

The canister state cannot be safely upgraded to the newer version.")
}

fn get_type_definition(state_header: &[u8]) -> Result<candid::TypeEnv, String> {
    use binread::BinRead;
    use candid::binary_parser::Header;
    use std::io::Cursor;

    let mut with_magic = vec![];
    with_magic.extend(MAGIC);
    with_magic.extend(state_header);

    let mut reader = Cursor::new(&with_magic);
    let header = Header::read(&mut reader).map_err(|e| e.to_string())?;
    let (env, _) = header.to_types().map_err(|e| e.to_string())?;

    Ok(env)
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
