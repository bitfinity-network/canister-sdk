use std::sync::Arc;

use candid::utils::ArgumentEncoder;
use candid::{CandidType, Decode, Principal};
use ic_exports::pocket_ic::*;
use serde::de::DeserializeOwned;

use crate::{CanisterClient, CanisterClientResult};

/// A client for interacting with a canister inside dfinity's PocketIc test framework.
#[derive(Clone)]
pub struct PocketIcClient {
    client: Option<Arc<PocketIc>>,
    pub canister: Principal,
    pub caller: Principal,
}

impl PocketIcClient {
    /// Creates a new instance of a PocketIcClient.
    /// The new instance is independent and have no access to canisters of other instances.
    pub async fn new(canister: Principal, caller: Principal) -> Self {
        Self::from_client(PocketIc::new().await, canister, caller)
    }

    /// Crates new instance of PocketIcClient from an existing client instance.
    pub fn from_client<P: Into<Arc<PocketIc>>>(
        client: P,
        canister: Principal,
        caller: Principal,
    ) -> Self {
        Self {
            client: Some(client.into()),
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

        let reply = if self.is_live() {
            let id = self
                .client()
                .submit_call(self.canister, self.caller, method, args)
                .await?;
            self.client().await_call_no_ticks(id).await
        } else {
            self.client()
                .update_call(self.canister, self.caller, method, args)
                .await
        }?;

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

        let reply = self
            .client()
            .query_call(self.canister, self.caller, method, args)
            .await?;

        let decoded = Decode!(&reply, R)?;
        Ok(decoded)
    }

    /// Returns true if the client is live.
    fn is_live(&self) -> bool {
        self.client
            .as_ref()
            .map(|client| client.url().is_some())
            .unwrap_or_default()
    }
}

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
