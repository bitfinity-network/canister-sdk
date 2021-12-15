use std::path::{Path, PathBuf};

use ic_agent::agent::http_transport::ReqwestHttpReplicaV2Transport;
use ic_agent::identity::BasicIdentity;
use ic_agent::Agent;

use crate::Result;

fn get_identity_path(account_name: impl AsRef<Path>) -> Result<PathBuf> {
    let mut path = dirs::config_dir().ok_or(crate::Error::MissingConfig)?;
    path.push("dfx/identity");
    path.push(account_name);
    path.push("identity.pem");
    Ok(path)
}

pub fn get_identity(account_name: impl AsRef<str>) -> Result<BasicIdentity> {
    let ident_path = get_identity_path(account_name.as_ref())?;
    let identity = BasicIdentity::from_pem_file(ident_path)?;
    Ok(identity)
}

pub async fn get_agent(name: impl AsRef<str>, url: impl Into<String>) -> Result<Agent> {
    let identity = get_identity(name)?;

    let transport = ReqwestHttpReplicaV2Transport::create(url)?;

    let agent = Agent::builder()
        .with_transport(transport)
        .with_identity(identity)
        .build()?;

    agent.fetch_root_key().await?;

    Ok(agent)
}
