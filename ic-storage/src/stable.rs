#![deny(missing_docs)]
//! This module provides versioned data for stable storage.
//!
//! **IMPORTANT**: do note that it's not possible to store more than one type (and one instance of that type)
//! in stable storage. Any subsequent writes will overwrite what is currently stored.
//!
//! This library makes it possible to change the type that is serialized and written to stable storage.
//!
//! Versioning happens between types, not data.
//! This means that any data written to stable storage through the `write` function
//! will overwrite whatever data was stored there from before.
//!
//! To be able to read and write a struct from stable storage
//! using these functions, the struct needs to implement the [`Versioned`] trait
//! (which in turn requires [`Deserialize`] and [`CandidType`])
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
//! ## Examples
//!
//! ### Creating a versioned struct
//! ```
//! use ic_storage::stable::Versioned;
//! use candid::CandidType;
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
//! # use candid::CandidType;
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
//! # use candid::CandidType;
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

#[cfg(target_arch = "wasm32")]
use ic_exports::ic_cdk::api::stable::{stable_bytes, stable_read, stable_size, StableWriter};
use ic_exports::candid::de::IDLDeserialize;
use ic_exports::candid::ser::IDLBuilder;
use ic_exports::candid::types::CandidType;
use serde::Deserialize;

#[cfg(not(target_arch = "wasm32"))]
use crate::testing::{stable_bytes, stable_read, stable_size, StableWriter};
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
    fn version() -> u32 {
        Self::Previous::version() + 1
    }

    /// Upgrade to this version from the previous version.
    fn upgrade(previous: Self::Previous) -> Self;
}

// -----------------------------------------------------------------------------
//     - Versioned implementation for a unit -
//     This is useful for the first version of a struct,
//     as we can set the `Previous` version of that implementation to a unit,
//     since a unit can never have a version lower than zero.
//
//     Trying to upgrade TO a unit (rather than FROM a unit) will panic!
// -----------------------------------------------------------------------------
impl Versioned for () {
    type Previous = ();

    fn version() -> u32 {
        0
    }

    fn upgrade((): ()) -> Self {
        panic!("It's not possible to upgrade to a unit, only from");
    }
}

fn read_version() -> Result<u32> {
    let mut version = [0u8; VERSION_SIZE];
    if ((stable_size() << 16) as usize) < version.len() {
        return Err(Error::InsufficientSpace);
    }

    stable_read(0, &mut version);
    Ok(u32::from_ne_bytes(version))
}

/// Load a [`Versioned`] from stable storage.
pub fn read<T: Versioned>() -> Result<T> {
    let version = read_version()?;
    if T::version() < version {
        return Err(Error::AttemptedDowngrade);
    }

    let bytes = stable_bytes();
    let res = recursive_upgrade::<T>(version, &bytes[VERSION_SIZE..])?;
    Ok(res)
}

/// Write a [`Versioned`] to stable storage.
/// This will overwrite anything that was previously stored, however
/// it is not allowed to write an older version than what is currently stored.
pub fn write<T: Versioned>(payload: &T) -> Result<()> {
    let current_version = match read_version() {
        Ok(v) => Some(v),
        Err(Error::InsufficientSpace) => None,
        Err(e) => return Err(e),
    };

    let version = T::version();

    if let Some(current_version) = current_version {
        if current_version > version {
            return Err(Error::ExistingVersionIsNewer);
        }
    }

    let mut writer = StableWriter::default();

    // Write the version number to the first four bytes.
    // There is no point in checking that the four bytes were actually
    // written as one would normally do with with an `io::Write`,
    // as the failure point lies in growing the stable storage,
    // and the usize returned from the `write` call isn't actually the number
    // of bytes written, but rather the size of the slice that was passed in.
    writer.write(&version.to_ne_bytes())?;

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
    use candid::CandidType;

    use super::*;

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

    #[test]
    fn write_and_upgrade() {
        let first = Version1(42);
        write(&first).unwrap();

        let Version2(a, b) = read::<Version2>().unwrap();
        assert_eq!((a, b), (42, 5));
    }

    #[test]
    #[should_panic(expected = "insufficient space available")]
    fn try_read_unwritten() {
        // Try to read when no version has ever been written
        let err = read::<Version1>().unwrap_err();
        panic!("{err}");
    }

    #[test]
    fn try_to_downgrade() {
        // Downgrading is not currently supported
        let second = Version2(1, 2);
        write(&second).unwrap();

        let err = read::<Version1>().unwrap_err();
        assert!(matches!(err, Error::AttemptedDowngrade))
    }

    #[test]
    fn overwrite_current_version_with_current_version() {
        let v1 = Version1(1);
        write(&v1).unwrap();
        let v1 = Version1(2);
        write(&v1).unwrap();

        let v1 = read::<Version1>().unwrap();

        let actual = v1.0;
        let expected = 2;
        assert_eq!(expected, actual);
    }

    #[test]
    #[should_panic(expected = "existing version is newer")]
    fn write_an_older_version() {
        // Write a version that is older than the one that
        // currently exists in storage.
        write(&Version2(0, 0)).unwrap();
        let err = write(&Version1(1)).unwrap_err();
        panic!("{err}");
    }
}
