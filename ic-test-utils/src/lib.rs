#![deny(missing_docs)]
#![doc = include_str!("../README.md")]
use std::borrow::Cow;
use std::path::Path;

use candid::utils::ArgumentEncoder;
use candid::Principal;
use ic_agent::identity::{PemError, Secp256k1Identity};
pub use ic_agent::Agent;

mod errors;
pub use errors::{Error, Result};

pub mod canister;

pub use canister::{Canister, Management, ManagementCanister, Wallet, WalletCanister};

/// Get the identity for an account.
/// This is useful for testing.
///
/// If this is ever needed outside of `get_agent` just make this
/// function public.
pub fn get_identity(account_name: impl AsRef<Path>) -> Result<Secp256k1Identity> {
    let mut ident_path = dirs::home_dir().ok_or(crate::Error::MissingConfig)?;
    ident_path.push(".config");
    ident_path.push("dfx/identity");
    ident_path.push(account_name);
    ident_path.push("identity.pem");

    match Secp256k1Identity::from_pem_file(&ident_path) {
        Ok(identity) => Ok(identity),
        Err(PemError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(Error::CertNotFound(ident_path))
        }
        Err(err) => Err(Error::from(err)),
    }
}

/// Create a default `Delay` with a throttle of 500ms
/// and a timeout of five minutes.
pub fn get_waiter() -> garcon::Delay {
    garcon::Delay::builder()
        .throttle(std::time::Duration::from_millis(500))
        .timeout(std::time::Duration::from_secs(60 * 5))
        .build()
}

/// Create a canister and install
/// the provided byte code.
pub async fn create_canister<T: ArgumentEncoder>(
    agent: &Agent,
    account_name: impl AsRef<str>,
    bytecode: Cow<'_, [u8]>,
    arg: T,
    cycles: u64,
) -> Result<Principal> {
    let wallet = Canister::new_wallet(agent, account_name)?;
    let management = Canister::new_management(agent);
    let canister_id = wallet.create_canister(cycles, None).await?;
    management
        .install_code(agent, canister_id, bytecode, arg)
        .await?;
    Ok(canister_id)
}

/// Reinstall the code for a canister.
pub async fn reinstall_canister<T: ArgumentEncoder>(
    agent: &Agent,
    canister_id: Principal,
    bytecode: Cow<'_, [u8]>,
    arg: T,
) -> Result<()> {
    let management = Canister::new_management(agent);
    management
        .reinstall_code(agent, canister_id, bytecode, arg)
        .await?;
    Ok(())
}
