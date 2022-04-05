//! The IC Management Canister
//!
//! This module has been implemented based on interface spec of [`The IC Management Canister`].
//!
//! [`The IC Management Canister`]: https://sdk.dfinity.org/docs/interface-spec/index.html#ic-management-canister

use candid::utils::ArgumentEncoder;
use candid::{encode_args, CandidType, Nat, Principal};
use ic_canister::virtual_canister_call;
use ic_cdk::api;
use ic_cdk::api::call::RejectionCode;
use serde::{Deserialize, Serialize};
use std::convert::{AsRef, From};

pub type CanisterID = Principal;
pub type UserID = Principal;
pub type WasmModule = Vec<u8>;

#[derive(CandidType, Clone, Deserialize, Default)]
pub struct CanisterSettings {
    pub controllers: Option<Vec<Principal>>,
    pub compute_allocation: Option<Nat>,
    pub memory_allocation: Option<Nat>,
    pub freezing_threshold: Option<Nat>,
}

#[derive(CandidType, Clone, Deserialize, Default)]
pub struct DefiniteCanisterSettings {
    pub controllers: Vec<Principal>,
    pub compute_allocation: Nat,
    pub memory_allocation: Nat,
    pub freezing_threshold: Nat,
}

#[derive(CandidType, Deserialize)]
pub enum InstallCodeMode {
    #[serde(rename = "install")]
    Install,
    #[serde(rename = "reinstall")]
    Reinstall,
    #[serde(rename = "upgrade")]
    Upgrade,
}

#[derive(CandidType, Deserialize, Serialize)]
pub enum CanisterStatusKind {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "stopping")]
    Stopping,
    #[serde(rename = "stopped")]
    Stopped,
}

#[derive(CandidType, Deserialize)]
pub struct CanisterStatus {
    pub status: CanisterStatusKind,
    pub settings: DefiniteCanisterSettings,
    pub module_hash: Option<Vec<u8>>,
    pub memory_size: Nat,
    pub cycles: Nat,
}

#[derive(CandidType, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Canister(CanisterID);

#[derive(CandidType, Deserialize)]
pub struct CreateCanisterInput {
    pub settings: Option<CanisterSettings>,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct CanisterIDArg {
    pub canister_id: CanisterID,
}

#[derive(CandidType, Deserialize)]
struct UpdateSettingsInput {
    pub canister_id: Principal,
    pub settings: CanisterSettings,
}

#[derive(CandidType, Deserialize)]
pub struct InstallCodeInput {
    pub mode: InstallCodeMode,
    pub canister_id: CanisterID,
    pub wasm_module: WasmModule,
    pub arg: Vec<u8>,
}

#[derive(CandidType, Deserialize)]
struct ProvisionalCreateCanisterWithCyclesInput {
    pub amount: Option<Nat>,
    pub settings: Option<CanisterSettings>,
}

#[derive(CandidType, Deserialize)]
struct ProvisionalTopUpCanisterInput {
    pub canister_id: CanisterID,
    pub amount: Nat,
}

impl Canister {
    pub async fn create(
        settings: Option<CanisterSettings>,
        cycles: u64, // TODO: strange, my analyzer says that this is not used :thinking:
    ) -> Result<Self, (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "create_canister",
            (CreateCanisterInput { settings },),
            CanisterIDArg,
            cycles
        )
        .await
        .map(|r| Self(r.canister_id))
    }

    /// A helper method to accept cycles from caller.
    pub fn accept_cycles() -> u64 {
        let amount = api::call::msg_cycles_available();
        if amount == 0 {
            return 0;
        }
        api::call::msg_cycles_accept(amount) // TODO: mock?
    }

    pub async fn provisional_create_with_cycles(
        amount: u64,
        settings: Option<CanisterSettings>,
    ) -> Result<Self, (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "provisional_create_canister_with_cycles",
            (ProvisionalCreateCanisterWithCyclesInput {
                amount: Some(Nat::from(amount)),
                settings,
            },),
            CanisterIDArg,
            amount
        )
        .await
        .map(|r| Self(r.canister_id))
    }

    pub async fn update_settings(
        &self,
        settings: CanisterSettings,
    ) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "update_settings",
            (UpdateSettingsInput {
                canister_id: self.0,
                settings,
            },),
            ()
        )
        .await
    }

    pub async fn install_code<T: ArgumentEncoder>(
        &self,
        mode: InstallCodeMode,
        wasm_module: WasmModule,
        arg: T,
    ) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "install_code",
            (InstallCodeInput {
                mode,
                canister_id: self.0,
                wasm_module,
                arg: encode_args(arg).unwrap_or_default(),
            },),
            ()
        )
        .await
    }

    pub async fn uninstall_code(&self) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "uninstall_code",
            (self.as_canister_id_arg(),),
            ()
        )
        .await
    }

    pub async fn start(&self) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "start_canister",
            (self.as_canister_id_arg(),),
            ()
        )
        .await
    }

    pub async fn stop(&self) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "stop_canister",
            (self.as_canister_id_arg(),),
            ()
        )
        .await
    }

    pub async fn status(&self) -> Result<CanisterStatus, (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "canister_status",
            (self.as_canister_id_arg(),),
            CanisterStatus
        )
        .await
    }

    pub async fn delete(&self) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "delete_canister",
            (self.as_canister_id_arg(),),
            ()
        )
        .await
    }

    pub async fn deposit_cycles(&self) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "deposit_cycles",
            (self.as_canister_id_arg(),),
            ()
        )
        .await
    }

    pub async fn raw_rand(&self) -> Result<Vec<u8>, (RejectionCode, String)> {
        virtual_canister_call!(Principal::management_canister(), "raw_rand", (), Vec<u8>).await
    }

    pub async fn provisional_top_up(&self, amount: Nat) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "provisional_top_up_canister",
            (ProvisionalTopUpCanisterInput {
                canister_id: self.0,
                amount,
            },),
            ()
        )
        .await
    }

    fn as_canister_id_arg(&self) -> CanisterIDArg {
        CanisterIDArg {
            canister_id: self.0,
        }
    }
}

impl AsRef<CanisterID> for Canister {
    fn as_ref(&self) -> &CanisterID {
        &self.0
    }
}

impl From<Canister> for CanisterID {
    fn from(canister: Canister) -> Self {
        canister.0
    }
}

impl From<CanisterID> for Canister {
    fn from(id: CanisterID) -> Self {
        Self(id)
    }
}
