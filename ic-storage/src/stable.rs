#![deny(missing_docs)]
//! This module provides versioned data for stable storage.
//!
//! This makes it possible to change the type that is serialized and written to stable storage.
//!
//! Versioning happens between types, not data.
//! This means that any data written to stable storage through the `write` function
//! will overwrite whatever data was stored there from before.
//!
//! To be able to read and write a struct from stable storage
//! using these functions, the struct needs to implement the [`Versioned`] trait
//! (which in turn requires [`Deserialize`] and [`CandidType`])
//!
//! How is the data stored in stable storage?
//!
//! The first four bytes written is the version number as a [`u32`],
//! the remaining bytes represent the struct.
//! ```text
//!  0 1 2 3 ...
//! +-+-+-+-+-+-+-+-+-+
//! |V|E|R|S|  Struct |
//! +-+-+-+-+-+-+-+-+-+
//! ```
//!
//! ## How to use this
//!
//! Write the first version to stable storage using `write`
//! on the `#[init]` method of the canister.
//!
//! ```ignore
//! #[init]
//! fn init() {
//!     let first_version = get_first_version();
//!     if let Err(e) = write(&first_version) {
//!         ic_cdk::eprintln!("Failed to write to stable storage: {}", e);
//!     }
//! }
//!
//! ```
//!
//! Once a new version of the struct has been created set the `Previous` associated type
//! to the previous version of the struct.
//!
//! ```ignore
//! impl Versioned for NewVersion {
//!     type Previous = OldVersion;
//!     ...
//! }
//! ```
//!
//! Make sure the previous version is written to stable storage before the upgrade.
//! On `post_upgrade` the new version can be populated from the previous version.
//!
//! ```ignore
//! #[pre_upgrade]
//! fn pre_upgrade() {
//!     let first_version: FirstVersion = get_first_version();
//!     write(first_version).unwrap();
//! }
//!
//! #[post_upgrade]
//! fn post_upgrade() {
//!     let second_version = read::<SecondVersion>().unwrap();
//! }
//! ```
//!
//! The old version still has to exist, this can be managed by numbering modules,
//! e.g `crate::v1::MyData`, `crate::v2::MyData`.
//!
//! Upgrades can span multiple versions, making it possible to upgrade from v1 to v3 in one go.
//!
//! **Note**: trying to load a version older than what is currently stored will result in an `AttemptedDowngrade` error.
//!
//! ## Examples
//!
//! ### Creating a versioned struct
//! ```
//! use ic_storage::stable::Versioned;
//! use ic_cdk::export::candid::CandidType;
//! use serde::Deserialize;
//!
//! #[derive(Debug, Deserialize, CandidType)]
//! struct First(usize, usize);
//!
//! impl Versioned for First {
//!     type Previous = ();
//!
//!     fn version() -> u32 { 1 }
//!
//!     fn upgrade((): ()) -> Self {
//!         First(0, 0)
//!     }
//! }
//!
//! #[derive(Debug, Deserialize, CandidType)]
//! struct Second(String);
//!
//! impl Versioned for Second {
//!     type Previous = First;
//!
//!     fn version() -> u32 { 2 }
//!
//!     fn upgrade(previous: Self::Previous) -> Self {
//!         Second(format!("{}, {}", previous.0, previous.1))
//!     }
//! }
//! ```
//!
//! ## Reading
//!
//! Read the latest implementation from stable memory.
//! If there is a previous version stored, this version will be upgraded.
//!
//! ```
//! use ic_storage::stable::{Versioned, read};
//! # use ic_cdk::export::candid::CandidType;
//! # use serde::Deserialize;
//!
//! # #[derive(Debug, Deserialize, CandidType)]
//! # struct First(usize, usize);
//! # impl Versioned for First {
//! #     type Previous = ();
//! #     fn version() -> u32 { 1 }
//! #     fn upgrade((): ()) -> Self {
//! #         First(0, 0)
//! #     }
//! # }
//! # #[derive(Debug, Deserialize, CandidType)]
//! # struct Second(String);
//! # impl Versioned for Second {
//! #     type Previous = First;
//! #     fn version() -> u32 { 2 }
//! #     fn upgrade(previous: Self::Previous) -> Self {
//! #         Self(format!("{}, {}", previous.0, previous.1))
//! #     }
//! # }
//! // #[post_upgrade]
//! fn post_upgrade_canister() {
//!     let second = read::<Second>().unwrap();
//! }
//!
//! ```
//!
//! ## Writing
//!
//! Write the current version to stable storage.
//!
//! ```
//! use ic_storage::stable::{Versioned, write};
//! # use ic_cdk::export::candid::CandidType;
//! # use serde::Deserialize;
//!
//! # #[derive(Debug, Deserialize, CandidType)]
//! # struct First(usize, usize);
//! # impl Versioned for First {
//! #     type Previous = ();
//! #     fn version() -> u32 { 1 }
//! #     fn upgrade((): ()) -> Self {
//! #         First(0, 0)
//! #     }
//! # }
//! # fn get_first() -> First { First(1, 2) }
//! // #[pre_upgrade]
//! fn pre_upgrade_canister() {
//!     let first: First = get_first();
//!     write(&first).unwrap();
//! }
//! ```
use std::mem::size_of;

use ic_cdk::api::stable::{stable_bytes, stable_read, stable_size, StableWriter};
use ic_cdk::export::candid::de::IDLDeserialize;
use ic_cdk::export::candid::ser::IDLBuilder;
use ic_cdk::export::candid::types::CandidType;
use serde::Deserialize;

use crate::{Error, Result};

const VERSION_SIZE: usize = size_of::<u32>();

/// Versioned data that can be written to, and read from stable storage.
pub trait Versioned: for<'de> Deserialize<'de> + CandidType {
    /// The previous version of this data.
    /// If there is no previous version specify a unit (`()`).
    /// This is required until defaults for associated types are stable.
    ///
    /// A unit has a version number of zero, and a `Previous` type of a unit,
    /// which means it's not possible to upgrade a unit from anything but it self,
    /// and calling `upgrade` will simply return `()`.
    type Previous: Versioned;

    /// The version of the data
    fn version() -> u32;

    /// Upgrade to this version from the previous version.
    fn upgrade(previous: Self::Previous) -> Self;
}

// -----------------------------------------------------------------------------
//     - Versioned implementation for a unit -
//     This is useful
// -----------------------------------------------------------------------------
impl Versioned for () {
    type Previous = ();

    fn version() -> u32 {
        0
    }

    fn upgrade((): ()) -> Self {}
}

/// Load a [`Versioned`] from stable storage.
pub fn read<T: Versioned>() -> Result<T> {
    let mut version = [0u8; VERSION_SIZE];
    if ((stable_size() << 16) as usize) < version.len() {
        return Err(Error::InsufficientSpace);
    }

    stable_read(0, &mut version);
    let version = u32::from_ne_bytes(version);
    if T::version() < version {
        return Err(Error::AttemptedDowngrade);
    }

    let bytes = stable_bytes();
    let res = recursive_upgrade::<T>(version, &bytes[VERSION_SIZE..])?;
    Ok(res)
}

/// Write a [`Versioned`] to stable storage.
/// This will overwrite anything that was previously stored.
pub fn write<T: Versioned>(payload: &T) -> Result<()> {
    let version = T::version().to_ne_bytes();
    let mut writer = StableWriter::default();

    // Write the version number to the first four bytes.
    // There is no point in checking that the four bytes were actually
    // written as one would normally do with with an `io::Write`,
    // as the failure point lies in growing the stable storage,
    // and the usize returned from the `write` call isn't actually the number
    // of bytes written, but rather the size of the slice that was passed in.
    writer.write(&version)?;

    // Serialize and write the `Versioned`
    let mut serializer = IDLBuilder::new();
    serializer.arg(payload)?.serialize(writer)?;

    Ok(())
}

// -----------------------------------------------------------------------------
//     - Recursively upgrade -
//     Recursively upgrade a `Versioned`.
// -----------------------------------------------------------------------------
fn recursive_upgrade<T: Versioned>(version: u32, bytes: &[u8]) -> Result<T> {
    if version == T::version() {
        let mut de = IDLDeserialize::new(bytes)?;
        let res = de.get_value()?;
        Ok(res)
    } else {
        let val = recursive_upgrade::<T::Previous>(version, bytes)?;
        Ok(T::upgrade(val))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ic_cdk::export::candid::CandidType;

    #[derive(Debug, CandidType, Deserialize)]
    struct Version1(u32);
    #[derive(Debug, CandidType, Deserialize)]
    struct Version2(u32, u32);
    #[derive(Debug, CandidType, Deserialize)]
    struct Version3(u32, u32, u32);

    impl Versioned for Version1 {
        type Previous = ();
        fn version() -> u32 {
            1
        }

        fn upgrade(_: Self::Previous) -> Self {
            Self(0)
        }
    }

    impl Versioned for Version2 {
        type Previous = Version1;
        fn version() -> u32 {
            2
        }

        fn upgrade(previous: Self::Previous) -> Self {
            Self(previous.0, 5)
        }
    }

    impl Versioned for Version3 {
        type Previous = Version2;
        fn version() -> u32 {
            3
        }

        fn upgrade(previous: Self::Previous) -> Self {
            Self(previous.0, previous.1, 900)
        }
    }

    #[test]
    fn upgrade_versions() {
        let mut v1_bytes = vec![];
        let mut serializer = IDLBuilder::new();
        serializer
            .arg(&Version1(1))
            .unwrap()
            .serialize(&mut v1_bytes)
            .unwrap();
        let v2 = super::recursive_upgrade::<Version2>(1, &v1_bytes).unwrap();
        let Version2(a, b) = v2;
        assert_eq!((a, b), (1, 5));
    }

    #[test]
    fn upgrade_across_two_versions() {
        let mut v1_bytes = vec![];
        let mut serializer = IDLBuilder::new();
        serializer
            .arg(&Version1(1))
            .unwrap()
            .serialize(&mut v1_bytes)
            .unwrap();
        let v3 = super::recursive_upgrade::<Version3>(1, &v1_bytes).unwrap();
        let Version3(a, b, c) = v3;
        assert_eq!((a, b, c), (1, 5, 900));
    }
}
