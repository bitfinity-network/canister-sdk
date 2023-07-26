pub use handler::*;
pub use interface::*;
pub use mock::*;

mod handler;
pub mod inject;
mod interface;
mod mock;
#[cfg(target_family = "wasm")]
mod wasm;

pub use candid;
pub use candid::Principal;
pub use ic_cdk::api::call::{CallResult, RejectionCode};
pub use ic_cdk_macros as macros;

/// A set of mock principal IDs useful for testing.
#[cfg(not(target_family = "wasm"))]
pub mod mock_principals {
    use crate::Principal;

    #[inline]
    pub fn alice() -> Principal {
        Principal::from_text("sgymv-uiaaa-aaaaa-aaaia-cai").unwrap()
    }

    #[inline]
    pub fn bob() -> Principal {
        Principal::from_text("ai7t5-aibaq-aaaaa-aaaaa-c").unwrap()
    }

    #[inline]
    pub fn john() -> Principal {
        Principal::from_text("hozae-racaq-aaaaa-aaaaa-c").unwrap()
    }

    #[inline]
    pub fn xtc() -> Principal {
        Principal::from_text("aanaa-xaaaa-aaaah-aaeiq-cai").unwrap()
    }
}

/// APIs/Methods to work with the Internet Computer.
pub mod ic;
/// The type definition of common canisters on the Internet Computer.
pub mod interfaces;

/// Return the IC context depending on the build target.
#[inline(always)]
#[deprecated(note = "get_context is deprecated use ic_kit::ic::*")]
pub fn get_context() -> &'static impl Context {
    #[cfg(not(target_family = "wasm"))]
    return inject::get_context();
    #[cfg(target_family = "wasm")]
    return wasm::IcContext::context();
}
