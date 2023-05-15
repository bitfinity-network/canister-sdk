//! `ic-payments` crate provides safe way to transfer ICRC-1 tokens to and from a canister.
//!
//! Most of the canisters that work with tokens require a safe and reliable way to receive from and
//! send tokens to the users, and receive information about such transactions. There are a few
//! challenges for implementing such transactions:
//! 1. When a user transfers tokens to the canister, the canister needs to be notified somehow
//!    that the transfer took place. ICRC-1 standard doesn't provide signed transaction info,
//!    and there's no method to receive a transaction by transaction ID.
//! 2. Sometimes transactions may fail due to IC networking issues or nodes being overloaded.
//!    A transparent retry mechanism can simplify client logic quite a bit.
//! 3. In rare cases IC may return an error for transaction, but the transaction actually succeeds
//!    (at least the IC specification does not guarantee that this can never happen). So some
//!    mechanism is required to recover from such kinds of errors.
//!
//! [`TokenTerminal`] class provides a generic methods to perform in and out transfers, dealing
//! with all three issues explained above. To create it a canister has to provide an implementation
//! for a [`Balances`] trait which stores the user balances in the canister.
//!
//! There are also convenience methods in [`icrc1`] module to call common operations of ICRC-1
//! compatible tokens.
//!
//! # Transfer types
//!
//! There are two [transfer types](transfer::TransferType) available for token terminal:
//! * Single-step transfer - performed with one inter-canister call an allows recovery during the
//!   deduplication period of the token (typically 24 hours). After deduplication period is over,
//!   the transfer cannot be recovered and would be considered failed.
//! * Double-step transfer - performed in 2 steps through an interim account unique for the
//!   transfer. Double-step transfer can be recovered at any time. Double-step transfer is
//!   considered complete when both steps of the transfer are finished successfully. It is
//!   considered failed only if the first step failed. If the first step is successful but the
//!   second step failed, the transfer is always kept in recovery list until it can be successfully
//!   completed (since the tokens are already locked in the interim account).
//!
//! # Performing a transfer
//!
//! General transfer execution is as follows:
//!
//! ```diagram
//!                       Execute transfer <-----------------------------
//!                              ↓                                      |
//!          ---------------- Success? -------------------              |
//!          ↓                   ↓                       ↓              |
//!         No                  Yes                   Unknown           |
//!          ↓                   ↓                       ↓              |
//!    Fail transfer     Complete transfer       Retry limit reached?   |
//!                                               ↓                ↓    |
//!                                              Yes               No ---
//!                                               ↓
//!                                         Save transfer
//!                                         to recovery list
//! ```
//!
//! All transactions sent to the token canisters contain the `created_at` field for deduplication,
//! meaning that retries can be performed safely.
//!
//! # Recovery
//!
//! Transfers stored in the recovery list can be recovered by calling
//! [`TokenTerminal::recover_all()`] method. There are two ways to recover a transfer, result of
//! which is not know to the terminal:
//!
//! 1. Using deduplication mechanism of ICRC-1 tokens. This mechanism is applied to all
//!    transfers that are recent enough, e.g. are initiated less than deduplication period of the
//!    token (typically 24 hours).
//! 2. Using interim accounts of double-step transfers. This mechanism can only be applied to the
//!    double-step transfers, and applied for transfers older than deduplication period.
//!
//! ## Recovery through deduplication
//!
//! If a transaction can be deduplicated, e.g. it's recent enough, then recovery attempt consists
//! of just sending a transaction with exactly same parameters to the token. If the token canister
//! returns `DuplicateTransaction` error, the token terminal can be sure that the transaction was
//! successful the first time, and then proceed to the transfer completion logic. Any other
//! response is handled as a normal transfer response.
//!
//! ## Recovery through interim account
//!
//! Double-step transfers are done through an interim account, unique for each transfer. This
//! allows the terminal to use that account balance to figure out whether the original transfer was
//! successful or not, and perform appropriate actions accordingly.
//!
//! ```diagram
//!                               Get interim account balance
//!                                           ↓
//!                   -------------------- Is it 0? ----------------------
//!                   ↓                                                  ↓
//!         -------- Yes ---------                           ----------- No ------------
//!         ↓                    ↓                           ↓                         ↓
//!     Recovering           Recovering                 Recovering                 Recovering
//!     first step           second step                first step                 second step
//!         ↓                    ↓                           ↓                         ↓
//!  Transfer failed,  Transfer was successful,    Transfer was successful,      Transfer failed,
//!  reject transfer      complete transfer           proceed with second      perform second step
//!                                                        step                     transfer
//!
//! # Token fee change
//!
//! Token terminal adds the fee value to all ICRC-1 transactions to make sure that the credited
//! amount actually equals the amount received at the target account. If the token fee
//! amount or minting account configuration changes without `this` canister knowing about it, all
//! transactions will fail with a `BadFee` error.
//!
//! In such case terminal will automatically retrieve updated configuration and retry the failed
//! transfer. The canister can also [set a callback](TokenTerminal::on_config_update) to be called
//! to update the configuration stored in the state.
//!
//! ```

use candid::{CandidType, Deserialize, Nat};
use ic_exports::ic_icrc1::Account;
use ic_exports::Principal;

mod balances;
pub mod error;
pub mod icrc1;
pub mod recovery_list;
mod token_terminal;
mod transfer;

pub use balances::*;
pub use error::PaymentError;
pub use recovery_list::*;
pub use token_terminal::*;
pub use transfer::*;

type Timestamp = u64;
type TxId = Nat;

/// Configuration of the token canister.
///
/// This configuration can be obtained by the [`icrc1::get_icrc1_configuration`] function.
#[derive(CandidType, Debug, Deserialize, Clone)]
pub struct TokenConfiguration {
    /// Principal of the token canister.
    pub principal: Principal,

    /// Transaction fee.
    pub fee: Nat,

    /// Token minting account.
    pub minting_account: Account,
}

impl TokenConfiguration {
    pub(crate) fn get_fee(&self, from_acc: &Account, to_acc: &Account) -> Nat {
        if *from_acc == self.minting_account || *to_acc == self.minting_account {
            0.into()
        } else {
            self.fee
        }
    }
}
