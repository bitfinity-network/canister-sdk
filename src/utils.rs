<<<<<<< HEAD
//! Utilities for testing
use std::path::Path;

use ic_agent::agent::http_transport::ReqwestHttpReplicaV2Transport;
use ic_agent::identity::BasicIdentity;
use ic_agent::Agent;
=======
use ic_agent::agent::http_transport::ReqwestHttpReplicaV2Transport;
use ic_agent::identity::BasicIdentity;
use ic_agent::Agent;
use std::env;
use std::path::PathBuf;
>>>>>>> 23876d9 (CPROD-300 add canister factory)

use crate::Result;

/// Get the identity for an account.
/// This is useful for testing.
pub fn get_identity(account_name: impl AsRef<Path>) -> Result<BasicIdentity> {
    let mut ident_path = dirs::config_dir().ok_or(crate::Error::MissingConfig)?;
    ident_path.push("dfx/identity");
    ident_path.push(account_name);
    ident_path.push("identity.pem");

    let identity = BasicIdentity::from_pem_file(ident_path)?;
    Ok(identity)
}

<<<<<<< HEAD
/// Get an agent by name.
/// This is assuming there is an agent identity available.
///
/// ```text
/// # Clone the identity project first
/// mkdir -p ~/.config/dfx/identity/
/// cp -Rn ./identity/.config/dfx/identity/* ~/.config/dfx/identity/
/// ```
pub async fn get_agent(name: impl AsRef<Path>, url: impl Into<String>) -> Result<Agent> {
    let identity = get_identity(name)?;
=======
pub fn get_identity(account_name: &str) -> BasicIdentity {
    BasicIdentity::from_pem_file(get_identity_path(account_name)).unwrap()
}
>>>>>>> 23876d9 (CPROD-300 add canister factory)

    let transport = ReqwestHttpReplicaV2Transport::create(url)?;

<<<<<<< HEAD
=======
    let t: ReqwestHttpReplicaV2Transport = ReqwestHttpReplicaV2Transport::create(url).unwrap();
>>>>>>> 23876d9 (CPROD-300 add canister factory)
    let agent = Agent::builder()
        .with_transport(transport)
        .with_identity(identity)
        .build()?;

    agent.fetch_root_key().await?;

    Ok(agent)
}
