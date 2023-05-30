//! The IC Management Canister
//!
//! This module has been implemented based on interface spec of [`The IC Management Canister`].
//!
//! [`The IC Management Canister`]: https://sdk.dfinity.org/docs/interface-spec/index.html#ic-management-canister

use std::convert::{AsRef, From};

use async_trait::async_trait;
use ic_canister::virtual_canister_call;
use ic_exports::ic_base_types::CanisterId;
use ic_exports::ic_cdk::api::call::RejectionCode;
use ic_exports::ic_cdk::export::candid::utils::ArgumentEncoder;
use ic_exports::ic_cdk::export::candid::{encode_args, CandidType, Nat, Principal};
use ic_exports::ic_ic00_types::{
    ECDSAPublicKeyArgs, ECDSAPublicKeyResponse, EcdsaCurve, EcdsaKeyId, SignWithECDSAArgs,
    SignWithECDSAReply, DerivationPath,
};
use ic_exports::ic_kit::ic;
use k256::pkcs8::{self, AlgorithmIdentifier, ObjectIdentifier, SubjectPublicKeyInfo};
use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};

use super::private::Sealed;
use crate::Pubkey;

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

#[async_trait]
pub trait ManagementPrincipalExt: Sealed {
    fn accept_cycles() -> u64;
    async fn create(
        settings: Option<CanisterSettings>,
        cycles: u64,
    ) -> Result<Principal, (RejectionCode, String)>;
    async fn provisional_create_with_cycles(
        amount: u64,
        settings: Option<CanisterSettings>,
    ) -> Result<Principal, (RejectionCode, String)>;
    async fn get_ecdsa_pubkey(
        canister_id: Option<CanisterId>,
        derivation_path: Vec<Vec<u8>>,
    ) -> Result<Pubkey, (RejectionCode, String)>;
    async fn sign_with_ecdsa(
        hash: &[u8; 32],
        derivation_path: Vec<Vec<u8>>,
    ) -> Result<SignWithECDSAReply, (RejectionCode, String)>;
    async fn update_settings(
        &self,
        settings: CanisterSettings,
    ) -> Result<(), (RejectionCode, String)>;
    async fn install_code<T: ArgumentEncoder + Send>(
        &self,
        mode: InstallCodeMode,
        wasm_module: WasmModule,
        arg: T,
    ) -> Result<(), (RejectionCode, String)>;
    async fn uninstall_code(&self) -> Result<(), (RejectionCode, String)>;
    async fn start(&self) -> Result<(), (RejectionCode, String)>;
    async fn stop(&self) -> Result<(), (RejectionCode, String)>;
    async fn status(&self) -> Result<CanisterStatus, (RejectionCode, String)>;
    async fn delete(&self) -> Result<(), (RejectionCode, String)>;
    async fn deposit_cycles(&self) -> Result<(), (RejectionCode, String)>;
    async fn raw_rand(&self) -> Result<Vec<u8>, (RejectionCode, String)>;
    async fn provisional_top_up(&self, amount: Nat) -> Result<(), (RejectionCode, String)>;
}

#[async_trait]
impl ManagementPrincipalExt for Principal {
    /// A helper method to accept cycles from caller.
    fn accept_cycles() -> u64 {
        let amount = ic::msg_cycles_available();
        if amount == 0 {
            return 0;
        }
        ic::msg_cycles_accept(amount)
    }

    #[allow(unused_variables)]
    async fn create(
        settings: Option<CanisterSettings>,
        cycles: u64,
    ) -> Result<Principal, (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "create_canister",
            (CreateCanisterInput { settings },),
            CanisterIDArg,
            cycles
        )
        .await
        .map(|canister_id| canister_id.canister_id)
    }

    async fn provisional_create_with_cycles(
        amount: u64,
        settings: Option<CanisterSettings>,
    ) -> Result<Principal, (RejectionCode, String)> {
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
        .map(|canister_id| canister_id.canister_id)
    }

    async fn get_ecdsa_pubkey(
        canister_id: Option<CanisterId>,
        derivation_path: Vec<Vec<u8>>,
    ) -> Result<Pubkey, (RejectionCode, String)> {
        let request = ECDSAPublicKeyArgs {
            canister_id,
            derivation_path: DerivationPath::new(derivation_path),
            key_id: EcdsaKeyId {
                curve: EcdsaCurve::Secp256k1,
                name: Default::default(),
            },
        };
        virtual_canister_call!(
            Principal::management_canister(),
            "ecdsa_public_key",
            (request,),
            ECDSAPublicKeyResponse
        )
        .await
        .map(|res| Pubkey::new(res.public_key))
    }

    async fn sign_with_ecdsa(
        hash: &[u8; 32],
        derivation_path: Vec<Vec<u8>>,
    ) -> Result<SignWithECDSAReply, (RejectionCode, String)> {
        let request = SignWithECDSAArgs {
            key_id: EcdsaKeyId {
                curve: EcdsaCurve::Secp256k1,
                name: Default::default(),
            },
            message_hash: *hash,
            derivation_path: DerivationPath::new(derivation_path),
        };
        virtual_canister_call!(
            Principal::management_canister(),
            "sign_with_ecdsa",
            (request,),
            ic_exports::ic_ic00_types::SignWithECDSAReply
        )
        .await
    }

    async fn update_settings(
        &self,
        settings: CanisterSettings,
    ) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "update_settings",
            (UpdateSettingsInput {
                canister_id: *self,
                settings,
            },),
            ()
        )
        .await
    }

    async fn install_code<T: ArgumentEncoder + Send>(
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
                canister_id: *self,
                wasm_module,
                arg: encode_args(arg).unwrap_or_default(),
            },),
            ()
        )
        .await
    }

    async fn uninstall_code(&self) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "uninstall_code",
            (CanisterIDArg { canister_id: *self },),
            ()
        )
        .await
    }

    async fn start(&self) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "start_canister",
            (CanisterIDArg { canister_id: *self },),
            ()
        )
        .await
    }

    async fn stop(&self) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "stop_canister",
            (CanisterIDArg { canister_id: *self },),
            ()
        )
        .await
    }

    async fn status(&self) -> Result<CanisterStatus, (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "canister_status",
            (CanisterIDArg { canister_id: *self },),
            CanisterStatus
        )
        .await
    }

    async fn delete(&self) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "delete_canister",
            (CanisterIDArg { canister_id: *self },),
            ()
        )
        .await
    }

    async fn deposit_cycles(&self) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "deposit_cycles",
            (CanisterIDArg { canister_id: *self },),
            ()
        )
        .await
    }

    async fn raw_rand(&self) -> Result<Vec<u8>, (RejectionCode, String)> {
        virtual_canister_call!(Principal::management_canister(), "raw_rand", (), Vec<u8>).await
    }

    async fn provisional_top_up(&self, amount: Nat) -> Result<(), (RejectionCode, String)> {
        virtual_canister_call!(
            Principal::management_canister(),
            "provisional_top_up_canister",
            (ProvisionalTopUpCanisterInput {
                canister_id: *self,
                amount,
            },),
            ()
        )
        .await
    }
}

#[derive(CandidType, Serialize, Deserialize, Debug)]
pub struct CallSignature {
    pub sender: Principal,
    pub recipient: Principal,
    pub request_id: Vec<u8>,
    pub content: Vec<u8>,
    pub status_request_content: Vec<u8>,
}

pub fn der_encode_pub_key(pk: &Pubkey) -> Vec<u8> {
    let pubkey = PublicKey::parse_slice(pk.as_bytes(), None).expect("not a valid public key");
    let pubkey_bytes_uncompress = pubkey.serialize();
    let der_encoded_public_key: pkcs8::Document = SubjectPublicKeyInfo {
        algorithm: AlgorithmIdentifier {
            oid: ObjectIdentifier::new_unwrap("1.2.840.10045.2.1"),
            parameters: Some((&ObjectIdentifier::new_unwrap("1.3.132.0.10")).into()),
        },
        subject_public_key: &pubkey_bytes_uncompress,
    }
    .try_into()
    .expect("not a valid PublicKeyDocument");
    der_encoded_public_key.as_ref().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_encode() {
        let input = "03981eff1934f035cce8df1a7182793fba2b9a5e96cfc423ca102902b60257c8fb";
        let bytes = hex::decode(input).unwrap();

        let expected = vec![
            48, 86, 48, 16, 6, 7, 42, 134, 72, 206, 61, 2, 1, 6, 5, 43, 129, 4, 0, 10, 3, 66, 0, 4,
            152, 30, 255, 25, 52, 240, 53, 204, 232, 223, 26, 113, 130, 121, 63, 186, 43, 154, 94,
            150, 207, 196, 35, 202, 16, 41, 2, 182, 2, 87, 200, 251, 208, 26, 138, 21, 221, 251,
            147, 43, 144, 216, 172, 31, 217, 124, 69, 205, 161, 89, 36, 6, 89, 203, 231, 134, 226,
            90, 62, 168, 242, 100, 183, 137,
        ];
        assert_eq!(der_encode_pub_key(&Pubkey::new(bytes)), expected);
    }
}
