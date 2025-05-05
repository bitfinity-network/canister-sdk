use std::future::Future;

use candid::utils::ArgumentEncoder;
use candid::CandidType;
use serde::de::DeserializeOwned;

use crate::CanisterClientResult;

/// Generic client for interacting with a canister.
/// This is used to abstract away the differences between the IC Agent and the
/// IC Canister.
/// The IC Agent is used for interaction through the dfx tool, while the IC
/// Canister is used for interacting with the EVM canister in wasm environments.
pub trait CanisterClient: Send + Clone {
    /// Call an update method on the canister.
    ///
    /// # Arguments
    ///
    /// * `method` - The method name.
    /// * `args` - The arguments to the method.
    ///
    /// # Returns
    ///
    /// The result of the method call.
    fn update<T, R>(
        &self,
        method: &str,
        args: T,
    ) -> impl Future<Output = CanisterClientResult<R>> + Send
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType;

    /// Call a query method on the canister.
    ///
    /// # Arguments
    ///
    /// * `method` - The method name.
    /// * `args` - The arguments to the method.
    ///
    /// # Returns
    ///
    /// The result of the method call.
    fn query<T, R>(
        &self,
        method: &str,
        args: T,
    ) -> impl Future<Output = CanisterClientResult<R>> + Send
    where
        T: ArgumentEncoder + Send + Sync,
        R: DeserializeOwned + CandidType;
}
