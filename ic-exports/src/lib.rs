// ----------------------------------- //
// Export dependencies from crates.io  //
// ----------------------------------- //

pub use candid; // this is needed for candid-derive macro exports
pub use ic_cdk::*;
pub use {ic_cdk, ic_cdk_macros, ic_cdk_timers, ic_kit, ic_stable_structures as stable_structures};

pub type BlockHeight = u64;

#[cfg(feature = "ledger")]
pub mod ledger {
    pub use ic_ledger_types::*;
}

// --------------------------------------------------- //
// Export dependencies from dfinity github repository  //
// --------------------------------------------------- //

// pub use {ic_base_types, ic_ic00_types::DerivationPath};


#[cfg(feature = "state-machine")]
pub use ic_state_machine_tests;
