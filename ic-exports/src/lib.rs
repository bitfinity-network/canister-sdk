//
// Export dependencies from crates.io
//

pub use candid; // this is needed for candid-derive macro exports
pub use ic_cdk::export::*;
pub use {
    ic_cdk, ic_cdk_macros, ic_cdk_timers, 
    ic_kit,
    ic_stable_structures as stable_structures,
};


pub type BlockHeight = u64;

//
// Export dependencies from dfinity github repository
//

pub use {
    cycles_minting_canister, ic_base_types, ic_crypto_sha,
    ic_ic00_types, ic_icrc1, ic_icrc1_index
};

#[cfg(feature = "ledger")]
pub mod ledger {
    pub use ic_ledger_core::Tokens;
    pub use icp_ledger::{
        AccountIdentifier, BinaryAccountBalanceArgs, CandidOperation, GetBlocksArgs,
        LedgerCanisterInitPayload, QueryBlocksResponse, Subaccount, TransferArgs, TransferError,
        TransferFee, TransferFeeArgs, DEFAULT_TRANSFER_FEE, TOKEN_SUBDIVIDABLE_BY,
    };
}
#[cfg(feature = "ledger")]
pub use icrc_ledger_types as icrc_types;

#[cfg(feature = "state-machine")]
pub use ic_state_machine_tests;
