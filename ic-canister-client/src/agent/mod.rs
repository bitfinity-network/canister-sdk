mod identity;

use std::path::{Path, PathBuf};

use candid::utils::ArgumentEncoder;
use candid::{encode_args, CandidType, Principal, Decode};
use ic_agent::identity::PemError;
use serde::Deserialize;
use thiserror::Error;

use crate::client::CanisterClient;
use crate::{CanisterClientError, CanisterClientResult};

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("agent error: {0}")]
    Agent(#[from] ic_agent::AgentError),

    #[error("failed to read PEM file {0}: {1}")]
    PemError(PathBuf, PemError),
}

pub type Result<T> = std::result::Result<T, AgentError>;

pub struct IcAgentClient {
    canister_id: Principal,
    agent: ic_agent::Agent,
}

impl IcAgentClient {
    /// Initialize an IC Agent with a PEM file
    pub async fn with_identity(
        canister: Principal,
        identity_path: &Path,
        network: &str,
    ) -> Result<Self> {
        let agent = identity::init_agent(identity_path, network).await?;
        Ok(Self {
            canister_id: canister,
            agent,
        })
    }

    /// Initialize an IC Agent with an existing agent
    pub fn with_agent(canister: Principal, agent: ic_agent::Agent) -> Self {
        Self {
            canister_id: canister,
            agent,
        }
    }
}

#[async_trait::async_trait]
impl CanisterClient for IcAgentClient {
    async fn query<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send,
        R: for<'de> Deserialize<'de> + CandidType,
    {
        let args = encode_args(args)?;

        self.agent
            .query(&self.canister_id, method)
            .with_arg(&args)
            .call()
            .await
            .map_err(CanisterClientError::IcAgentError)
            .map(|r| decode(&r))
    }

    async fn update<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send,
        R: for<'de> Deserialize<'de> + CandidType,
    {
        let args = encode_args(args)?;
        self.agent
            .update(&self.canister_id, method)
            .with_arg(&args)
            .call_and_wait()
            .await
            .map_err(CanisterClientError::IcAgentError)
            .map(|r| decode(&r))
    }
}

#[inline]
fn decode<'a, T: CandidType + Deserialize<'a>>(bytes: &'a [u8]) -> T {
    Decode!(bytes, T).expect("failed to decode item from candid")
}