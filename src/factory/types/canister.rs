use super::checksum::Version;
use candid::{CandidType, Nat, Principal};
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

    /// Creates a new canister and installs `wasm_module` on it.
    pub async fn create(version: Version, wasm_module: Vec<u8>, arg: Vec<u8>) -> CallResult<Self> {
        let principal = api::call::call(
            Principal::management_canister(),
            "create_canister",
            (CreateCanisterInput { settings: None },),
        )
        .await
        .map(|r: (CanisterIDArg,)| r.0.canister_id)?;

        api::call::call(
            Principal::management_canister(),
            "install_code",
            (InstallInputArgs {
                mode: InstallMode::Install,
                canister_id: principal,
                wasm_module,
                arg,
            },),
        )
        .await?;

        Ok(Self(principal, version))
    }

    /// Upgrades the canister.
    pub async fn upgrade(&mut self, version: Version, wasm_module: Vec<u8>) -> CallResult<()> {
        api::call::call(
            Principal::management_canister(),
            "install_code",
            (InstallInputArgs {
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

#[derive(CandidType, Clone, Deserialize, Default)]
pub struct CanisterSettings {
    pub controllers: Option<Vec<Principal>>,
    pub compute_allocation: Option<Nat>,
    pub memory_allocation: Option<Nat>,
    pub freezing_threshold: Option<Nat>,
}

#[derive(CandidType, Deserialize)]
struct CreateCanisterInput {
    pub settings: Option<CanisterSettings>,
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
struct InstallInputArgs {
    pub mode: InstallMode,
    pub canister_id: Principal,
    pub wasm_module: Vec<u8>,
    pub arg: Vec<u8>,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct CanisterIDArg {
    pub canister_id: Principal,
}
