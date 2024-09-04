use candid::utils::ArgumentEncoder;
use candid::{CandidType, Principal};
use serde::de::DeserializeOwned;

use crate::client::CanisterClient;
use crate::{CanisterClientError, CanisterClientResult};

/// This client is used to interact with the IC canister.
#[derive(Debug, Clone)]
pub struct IcCanisterClient {
    /// The canister id of the Evm canister
    pub canister_id: Principal,
}

impl IcCanisterClient {
    pub fn new(canister: Principal) -> Self {
        Self {
            canister_id: canister,
        }
    }

    async fn call<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send,
        R: DeserializeOwned + CandidType,
    {
        let call_result: Result<(R,), _> =
            ic_exports::ic_cdk::call(self.canister_id, method, args).await;
        call_result
            .map(|(r,)| r)
            .map_err(CanisterClientError::CanisterError)
    }
}

#[async_trait::async_trait]
impl CanisterClient for IcCanisterClient {
    async fn update<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType,
    {
        self.call(method, args).await
    }

    async fn query<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType,
    {
        self.call(method, args).await
    }
}
