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
    pub use icp_ledger::{
        AccountIdentifier, BinaryAccountBalanceArgs, CandidOperation, GetBlocksArgs,
        LedgerCanisterInitPayload, QueryBlocksResponse, Subaccount, Tokens, TransferArgs,
        TransferError, TransferFee, TransferFeeArgs, DEFAULT_TRANSFER_FEE, TOKEN_SUBDIVIDABLE_BY,
    };
}
#[cfg(feature = "ledger")]
pub use {
    ic_icrc1_ledger as icrc1_ledger, ic_ledger_canister_core, icrc_ledger_types as icrc_types,
};

#[cfg(feature = "state-machine")]
pub use ic_state_machine_tests;
