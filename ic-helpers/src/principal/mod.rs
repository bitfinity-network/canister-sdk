//! *Ext traits with additional api's for calling remote canisters implemented for `Principal`

// Seal the *Ext traits to be only implemented for `Principal`.
mod private {
    use ic_exports::ic_cdk::export::candid::Principal;
    pub trait Sealed {}
    impl Sealed for Principal {}
}

pub mod ledger;
pub mod management;
