//! *Ext traits with additional api's for calling remote canisters implemented for `Principal`

// Seal the *Ext traits to be only implemented for `Principal`.
mod private {
    use ic_exports::candid::Principal;
    pub trait Sealed {}
    impl Sealed for Principal {}
}

#[cfg(feature = "ledger")]
pub mod ledger;
#[cfg(feature = "management_canister")]
pub mod management;
