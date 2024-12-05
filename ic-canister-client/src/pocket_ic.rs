use std::sync::Arc;

use candid::utils::ArgumentEncoder;
use candid::{CandidType, Decode, Principal};
use ic_exports::ic_kit::RejectionCode;
use ic_exports::pocket_ic::*;
use serde::de::DeserializeOwned;

use crate::{CanisterClient, CanisterClientError, CanisterClientResult};

#[derive(Clone)]
/// A wrapper of [`PocketIc`] that can be either a reference or an owned instance.
enum PocketIcInstance<'a> {
    Ref(&'a PocketIc),
    Owned(Arc<PocketIc>),
}

impl From<Arc<PocketIc>> for PocketIcInstance<'_> {
    fn from(client: Arc<PocketIc>) -> Self {
        PocketIcInstance::Owned(client)
    }
}

impl<'a> From<&'a PocketIc> for PocketIcInstance<'a> {
    fn from(client: &'a PocketIc) -> Self {
        PocketIcInstance::Ref(client)
    }
}

impl AsRef<PocketIc> for PocketIcInstance<'_> {
    fn as_ref(&self) -> &PocketIc {
        match self {
            PocketIcInstance::Ref(client) => client,
            PocketIcInstance::Owned(client) => client,
        }
    }
}

/// A client for interacting with a canister inside dfinity's PocketIc test framework.
#[derive(Clone)]
pub struct PocketIcClient<'a> {
    client: PocketIcInstance<'a>,
    pub canister: Principal,
    pub caller: Principal,
}

impl<'a> PocketIcClient<'a> {
    /// Creates a new instance of a [`PocketIcClient`].
    /// The new instance is independent and have no access to canisters of other instances.
    pub async fn new(canister: Principal, caller: Principal) -> Self {
        Self {
            client: Arc::new(PocketIc::new().await).into(),
            canister,
            caller,
        }
    }

    /// Creates new instance of PocketIcClient from an owned existing [`PocketIc`] instance.
    pub fn from_client<P>(client: P, canister: Principal, caller: Principal) -> Self
    where
        P: Into<Arc<PocketIc>>,
    {
        Self {
            client: client.into().into(),
            canister,
            caller,
        }
    }

    /// Crates new instance of PocketIcClient from a borrowed existing [`PocketIc`] instance.
    pub fn from_ref(client: &'a PocketIc, canister: Principal, caller: Principal) -> Self {
        Self {
            client: client.into(),
            canister,
            caller,
        }
    }

    /// Returns the PocketIC client for the canister.
    pub fn client(&self) -> &PocketIc {
        self.client.as_ref()
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
impl CanisterClient for PocketIcClient<'_> {
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
