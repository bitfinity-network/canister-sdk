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
    live: bool,
}

impl PocketIcClient {
    /// Creates a new instance of a PocketIcClient.
    /// The new instance is independent and have no access to canisters of other instances.
    pub async fn new(canister: Principal, caller: Principal) -> Self {
        Self::from_client(PocketIc::new().await, canister, caller)
    }

    /// Creates a new instance of a PocketIcClient in live mode
    /// The new instance is independent and have no access to canisters of other instances.
    ///
    /// Live mode flag is required to use a different update call method.
    pub async fn new_live(canister: Principal, caller: Principal) -> Self {
        let mut client = PocketIc::new().await;
        client.make_live(None).await;

        Self::from_client(client, canister, caller)
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
            live: false,
        }
    }

    /// Crates new instance of PocketIcClient from an existing client instance with live mode set.
    ///
    /// Note: the passed client MUST already be in live mode.
    ///
    /// Live mode flag is required to use a different update call method.
    pub fn from_client_live<P: Into<Arc<PocketIc>>>(
        client: P,
        canister: Principal,
        caller: Principal,
    ) -> Self {
        Self {
            client: Some(client.into()),
            canister,
            caller,
            live: true,
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

        let reply = if self.live {
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
