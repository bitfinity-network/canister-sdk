// ----------------------------------- //
// Export dependencies from crates.io  //
// ----------------------------------- //

pub use candid; // this is needed for candid-derive macro exports
pub use ic_cdk::export::*;
pub use {ic_cdk, ic_cdk_macros, ic_cdk_timers, ic_kit, ic_stable_structures as stable_structures};

pub type BlockHeight = u64;

// --------------------------------------------------- //
// Export dependencies from dfinity github repository  //
// --------------------------------------------------- //

pub use {ic_base_types, ic_ic00_types};

#[cfg(feature = "ledger")]
pub mod ledger {
    pub use ic_ledger_types::{
        AccountIdentifier, AccountBalanceArgs, GetBlocksArgs,
        QueryBlocksResponse, Subaccount, Tokens, TransferArgs,
        TransferError, DEFAULT_FEE,
    };
}
#[cfg(feature = "ledger")]
pub use {
    ic_icrc1_ledger as icrc1_ledger, ic_ledger_canister_core, icrc_ledger_types as icrc_types,
};

#[cfg(feature = "state-machine")]
pub use ic_state_machine_tests;
