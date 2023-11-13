use candid::utils::ArgumentEncoder;
use candid::CandidType;
use serde::Deserialize;

use crate::CanisterClientResult;

/// Generic client for interacting with a canister.
/// This is used to abstract away the differences between the IC Agent and the
/// IC Canister.
/// The IC Agent is used for interaction through the dfx tool, while the IC
/// Canister is used for interacting with the EVM canister in wasm environments.
#[async_trait::async_trait]
pub trait CanisterClient: Clone {
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
    async fn update<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: for<'de> Deserialize<'de> + CandidType;

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
    async fn query<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send + Sync,
        R: for<'de> Deserialize<'de> + CandidType;
}
