use ic_exports::ic_cdk::api::call::CallResult;
use ic_exports::ic_cdk::export::candid::utils::ArgumentEncoder;
use ic_exports::ic_cdk::export::candid::{CandidType, Principal};
use ic_helpers::management::{InstallCodeMode, ManagementPrincipalExt};
use serde::{Deserialize, Serialize};

use super::checksum::Version;

/// Represents information of a canister.
#[derive(CandidType, Debug, Clone, Serialize, Deserialize)]
pub struct Canister(Principal, Version);

impl Canister {
    pub fn new(id: Principal, version: Version) -> Self {
        Self(id, version)
    }

    /// Returns the version of module that is installed on canister.
    pub fn version(&self) -> Version {
        self.1
    }

    /// Returns the principal id of canister.
    pub fn identity(&self) -> Principal {
        self.0
    }

    /// Creates a new canister and installs `wasm_module` on it.
    pub async fn create<A: ArgumentEncoder + Send>(
        version: Version,
        wasm_module: Vec<u8>,
        arg: A,
        cycles: u64,
    ) -> CallResult<Self> {
        let canister = <Principal as ManagementPrincipalExt>::create(None, cycles).await?;
        canister
            .install_code(InstallCodeMode::Install, wasm_module, arg)
            .await?;
        Ok(Self(canister, version))
    }

    /// Upgrades the canister.
    pub async fn upgrade(&mut self, version: Version, wasm_module: Vec<u8>) -> CallResult<()> {
        self.0
            .install_code(InstallCodeMode::Upgrade, wasm_module, ())
            .await?;
        self.1 = version;
        Ok(())
    }
}
