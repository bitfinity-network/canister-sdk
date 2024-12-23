pub use candid; // this is needed for candid-derive macro exports
pub use {ic_cdk, ic_cdk_macros, ic_cdk_timers, ic_kit};

pub type BlockHeight = u64;

#[cfg(feature = "ledger")]
pub mod ledger {
    pub use ic_ledger_types::*;
}

#[cfg(feature = "icrc")]
mod icrc1_ledger;

#[cfg(feature = "icrc")]
pub mod icrc_types {
    pub use icrc_ledger_types::*;
    pub mod icrc1_ledger {
        pub use crate::icrc1_ledger::*;
    }
}

#[cfg(feature = "pocket-ic-tests")]
pub mod pocket_ic;
