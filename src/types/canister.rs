use super::checksum::Version;
use candid::{CandidType, Principal};
use ic_cdk::api;
use ic_cdk::api::call::CallResult;
use serde::{Deserialize, Serialize};

/// Represents information of a canister.
#[derive(CandidType, Clone, Serialize, Deserialize)]
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

    /// Upgrades the canister.
    pub async fn upgrade(&mut self, version: Version, wasm_module: Vec<u8>) -> CallResult<()> {
        api::call::call(
            Principal::management_canister(),
            "install_code",
            (UpgradeInputArgs {
                mode: InstallMode::Upgrade,
                canister_id: self.0,
                wasm_module,
                arg: candid::encode_args(()).unwrap_or_default(),
            },),
        )
        .await?;
        self.1 = version;
        Ok(())
    }
}

#[derive(CandidType, Deserialize)]
enum InstallMode {
    #[serde(rename = "install")]
    Install,
    #[serde(rename = "reinstall")]
    Reinstall,
    #[serde(rename = "upgrade")]
    Upgrade,
}

#[derive(CandidType, Deserialize)]
struct UpgradeInputArgs {
    pub mode: InstallMode,
    pub canister_id: Principal,
    pub wasm_module: Vec<u8>,
    pub arg: Vec<u8>,
}
