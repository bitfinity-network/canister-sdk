use candid::utils::ArgumentEncoder;
use candid::{CandidType, Decode, Principal};
use ic_exports::ic_kit::RejectionCode;
use ic_exports::pocket_ic::*;
use serde::de::DeserializeOwned;

use crate::{CanisterClient, CanisterClientError, CanisterClientResult};

/// A client for interacting with a canister inside dfinity's PocketIc test framework.
pub struct PocketIcClient {
    client: Option<PocketIc>,
    pub canister: Principal,
    pub caller: Principal,
}

impl Drop for PocketIcClient {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            // Spawns a tokio task to drop the client.
            // This workaround is necessary because Rust does not support async drop.
            //
            // This has two main drawbacks:
            //
            // 1. The tokio task is blocked while the client is dropped.
            // 2. It panics if not executed in a tokio runtime.
            //
            tokio::spawn(async move {
                client.drop().await;
            });
        }
    }
}

impl PocketIcClient {
    /// Creates a new instance of a PocketIcClient.
    /// The new instance is independent and have no access to canisters of other instances.
    pub async fn new(canister: Principal, caller: Principal) -> Self {
        Self {
            client: Some(PocketIc::new().await),
            canister,
            caller,
        }
    }

    /// Crates new instance of PocketIcClient from an existing client instance.
    pub fn from_client(client: PocketIc, canister: Principal, caller: Principal) -> Self {
        Self {
            client: Some(client),
            canister,
            caller,
        }
    }

    /// Returns the PocketIC client for the canister.
    pub fn client(&self) -> &PocketIc {
        self.client
            .as_ref()
            .expect("PocketIC client is not available")
    }

    /// Performs update call with the given arguments.
    pub async fn update<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType,
    {
        let args = candid::encode_args(args)?;

        let call_result = self
            .client()
            .update_call(self.canister, self.caller, method, args)
            .await?;

        let reply = match call_result {
            WasmResult::Reply(reply) => reply,
            WasmResult::Reject(e) => return Err(reject_error(e)),
        };

        let decoded = Decode!(&reply, R)?;
        Ok(decoded)
    }

    /// Performs query call with the given arguments.
    pub async fn query<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType,
    {
        let args = candid::encode_args(args)?;

        let call_result = self
            .client()
            .query_call(self.canister, self.caller, method, args)
            .await?;

        let reply = match call_result {
            WasmResult::Reply(reply) => reply,
            WasmResult::Reject(e) => return Err(reject_error(e)),
        };

        let decoded = Decode!(&reply, R)?;
        Ok(decoded)
    }
}

fn reject_error(e: String) -> CanisterClientError {
    CanisterClientError::CanisterError((RejectionCode::CanisterError, e))
}

#[async_trait::async_trait]
impl CanisterClient for PocketIcClient {
    async fn update<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType,
    {
        PocketIcClient::update(self, method, args).await
    }

    async fn query<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType,
    {
        PocketIcClient::query(self, method, args).await
    }
}
