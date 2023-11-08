//! Functions specific to the wallet.
//!
//! The [`Wallet`] should be used together with a [`Canister`].
//!
//! ```
//! # async fn run() {
//! use ic_test_utils::{get_agent, Canister};
//!
//! let user = "bob";
//! let agent = get_agent(user, None, None).await.unwrap();
//! let wallet = Canister::new_wallet(&agent, user);
//! # }
//! ```

use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

use candid::{CandidType, Decode, Deserialize, Encode, Principal};
use ic_agent::agent::UpdateBuilder;
use ic_agent::Agent;

use super::Canister;
use crate::{Error, Result};

const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(1_000); // milliseconds

/// Get the principal of a wallet.
fn get_wallet_principal(account_name: impl AsRef<str>) -> Result<Principal> {
    use_identity(account_name)?;
    let output = execute_command_with_retry("dfx", &["identity", "get-wallet"], MAX_RETRIES)?;
    let stdout = String::from_utf8(output.stdout).expect("invalid utf8");
    let principal = Principal::from_text(stdout.trim())?;
    Ok(principal)
}

/// Use an identity.
fn use_identity(account_name: impl AsRef<str>) -> Result<()> {
    execute_command_with_retry(
        "dfx",
        &["identity", "use", &account_name.as_ref().to_lowercase()],
        MAX_RETRIES,
    )?;
    Ok(())
}

/// Execute a command with retries.
fn execute_command_with_retry(command: &str, args: &[&str], max_retries: u32) -> Result<Output> {
    for retry in 0..=max_retries {
        match execute_command(command, args) {
            Ok(output) => {
                if output.status.success() {
                    return Ok(output);
                } else {
                    return Err(Error::InvalidOrMissingAccount);
                }
            }
            Err(_) if retry < max_retries => {
                thread::sleep(RETRY_DELAY);
            }
            Err(_) => return Err(Error::CommandExecutionFailed),
        }
    }
    Err(Error::CommandExecutionFailed)
}

/// Execute a command.
fn execute_command(command: &str, args: &[&str]) -> std::result::Result<Output, std::io::Error> {
    Command::new(command).args(args).output()
}

/// The balance result of a `Wallet::balance` call.
#[derive(Debug, CandidType, Deserialize)]
pub struct BalanceResult {
    pub amount: u64,
}

/// The result of a `Wallet::call_forward` call.
#[derive(Debug, CandidType, Deserialize)]
pub struct CallResult {
    #[serde(with = "serde_bytes")]
    #[serde(rename = "return")]
    pub payload: Vec<u8>,
}

#[derive(CandidType, Deserialize)]
pub struct CreateResult {
    pub canister_id: Principal,
}

#[derive(Debug, CandidType, Deserialize)]
struct CallForwardArgs {
    canister: Principal,
    method_name: String,
    #[serde(with = "serde_bytes")]
    args: Vec<u8>,
    cycles: u64,
}

/// Wallet for cycles
pub struct Wallet;

impl<'agent> Canister<'agent, Wallet> {
    /// Create a new wallet canister.
    /// If the `wallet_id_path` is `None` then the default [`WALLET_IDS_PATH`] will
    /// be used.
    pub fn new_wallet(agent: &'agent Agent, account_name: impl AsRef<str>) -> Result<Self> {
        let id = get_wallet_principal(account_name)?;
        let inst = Self::new(id, agent);
        Ok(inst)
    }

    /// Get the current balance of a canister
    pub async fn balance(&self) -> Result<BalanceResult> {
        let builder = self
            .agent
            .query(self.principal(), "wallet_balance")
            .with_arg(Encode!(&())?);
        let data = builder.call().await?;
        let balance = Decode!(&data, BalanceResult)?;
        Ok(balance)
    }

    /// Forward a call through the wallet, so cycles can be spent.
    pub async fn call_forward(&self, call: UpdateBuilder<'_>, cycles: u64) -> Result<Vec<u8>> {
        let call_forward_args = CallForwardArgs {
            canister: call.canister_id,
            method_name: call.method_name,
            args: call.arg,
            cycles,
        };
        let builder = self
            .agent
            .update(self.principal(), "wallet_call")
            .with_arg(Encode!(&call_forward_args)?);
        let data = builder.call_and_wait().await?;
        let val = Decode!(&data, std::result::Result<CallResult, String>)??;
        Ok(val.payload)
    }

    // There seem to be no use of compute allocation, memory allocation or freezing threshold.
    // If they are needed in the future we can add them as they are just newtypes around numbers,
    // and they should be sent along with the canister settings.
    /// Create an empty canister.
    /// This does not install the wasm code for the canister.
    /// To do that call [`Canister::install_code`] after creating a canister.
    pub async fn create_canister(
        &self,
        cycles: u64,
        controllers: impl Into<Option<Vec<Principal>>>,
    ) -> Result<Principal> {
        #[derive(Debug, CandidType, Deserialize)]
        struct In {
            cycles: u64,
            settings: CanisterSettings,
        }

        #[derive(Debug, CandidType, Deserialize)]
        struct CanisterSettings {
            controllers: Option<Vec<Principal>>,
            compute_allocation: Option<u8>,
            memory_allocation: Option<u64>,
            freezing_threshold: Option<u64>,
        }

        let mut builder = self
            .agent
            .update(self.principal(), "wallet_create_canister");
        let args = In {
            cycles,
            settings: CanisterSettings {
                controllers: controllers.into(),
                compute_allocation: None,
                memory_allocation: None,
                freezing_threshold: None,
            },
        };
        builder = builder.with_arg(Encode!(&args)?);
        let data = builder.call_and_wait().await?;
        let result = Decode!(&data, std::result::Result<CreateResult, String>)??;
        Ok(result.canister_id)
    }
}

// -----------------------------------------------------------------------------
//     - TODO -
//     Do we need even need these types?
// -----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
pub struct ComputeAllocation(u8);

impl std::convert::From<ComputeAllocation> for u8 {
    fn from(compute_allocation: ComputeAllocation) -> Self {
        compute_allocation.0
    }
}

macro_rules! try_from_compute_alloc_decl {
    ( $t: ty ) => {
        impl std::convert::TryFrom<$t> for ComputeAllocation {
            type Error = Error;

            fn try_from(value: $t) -> Result<Self> {
                if (value as i64) < 0 || (value as i64) > 100 {
                    Err(Error::MustBeAPercentage())
                } else {
                    Ok(Self(value as u8))
                }
            }
        }
    };
}

try_from_compute_alloc_decl!(u8);
try_from_compute_alloc_decl!(u16);
try_from_compute_alloc_decl!(u32);
try_from_compute_alloc_decl!(u64);
try_from_compute_alloc_decl!(i8);
try_from_compute_alloc_decl!(i16);
try_from_compute_alloc_decl!(i32);
try_from_compute_alloc_decl!(i64);

pub struct MemoryAllocation(u64);

impl std::convert::From<MemoryAllocation> for u64 {
    fn from(memory_allocation: MemoryAllocation) -> Self {
        memory_allocation.0
    }
}

macro_rules! try_from_memory_alloc_decl {
    ( $t: ty ) => {
        impl std::convert::TryFrom<$t> for MemoryAllocation {
            type Error = Error;

            fn try_from(value: $t) -> Result<Self> {
                if (value as i64) < 0 || (value as i64) > (1i64 << 48) {
                    Err(Error::InvalidMemorySize(value as u64))
                } else {
                    Ok(Self(value as u64))
                }
            }
        }
    };
}

try_from_memory_alloc_decl!(u8);
try_from_memory_alloc_decl!(u16);
try_from_memory_alloc_decl!(u32);
try_from_memory_alloc_decl!(u64);
try_from_memory_alloc_decl!(i8);
try_from_memory_alloc_decl!(i16);
try_from_memory_alloc_decl!(i32);
try_from_memory_alloc_decl!(i64);
