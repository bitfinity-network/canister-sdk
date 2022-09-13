//! The IC Management Canister
//!
//! This module has been implemented based on interface spec of [`The IC Management Canister`].
//!
//! [`The IC Management Canister`]: https://sdk.dfinity.org/docs/interface-spec/index.html#ic-management-canister

use crate::agent::request_id::to_request_id;
use crate::agent::{construct_message, read_state_content, update_content, Envelope};
use candid::types::ic_types::hash_tree::Label;
use candid::utils::ArgumentEncoder;
use candid::{encode_args, CandidType, Nat, Principal};
use ic_base_types::CanisterId;
use ic_canister::virtual_canister_call;
use ic_cdk::api::call::RejectionCode;
use ic_ic00_types::{ECDSAPublicKeyResponse, SignWithECDSAReply};
use k256::pkcs8::{self, AlgorithmIdentifier, ObjectIdentifier, SubjectPublicKeyInfo};
use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::convert::{AsRef, From};

use crate::Pubkey;

pub type CanisterID = Principal;
pub type UserID = Principal;
pub type WasmModule = Vec<u8>;

/// WARNING!!!
/// This struct is not imported from `ic00_types`
/// because `dfx 0.9.3` uses older version of this struct then
/// the `ic-ic00-types` dependency of this crate.
/// Code that uses this struct may not work on later versions of dfx.
#[derive(CandidType, Deserialize, Debug)]
pub struct ECDSAPublicKeyArgs {
    pub canister_id: Option<CanisterId>,
    pub derivation_path: Vec<Vec<u8>>,
    pub key_id: String,
}

/// WARNING!!!
/// This struct is not imported from `ic00_types`
/// because `dfx 0.9.3` uses older version of this struct then
/// the `ic-ic00-types` dependency of this crate.
/// Code that uses this struct may not work on later versions of dfx.
#[derive(CandidType, Deserialize, Debug)]
pub struct SignWithECDSAArgs {
    pub message_hash: Vec<u8>,
    pub derivation_path: Vec<Vec<u8>>,
    pub key_id: String,
}

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
    #[allow(unused_variables)]
    pub async fn create(
        settings: Option<CanisterSettings>,
        cycles: u64,
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
        let amount = ic_kit::ic::msg_cycles_available();
        if amount == 0 {
            return 0;
        }
        ic_kit::ic::msg_cycles_accept(amount)
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

    pub async fn get_ecdsa_pubkey(
        canister_id: Option<CanisterId>,
        derivation_path: Vec<Vec<u8>>,
    ) -> Result<Pubkey, (RejectionCode, String)> {
        let request = ECDSAPublicKeyArgs {
            canister_id,
            derivation_path,
            key_id: "secp256k1".into(),
        };
        virtual_canister_call!(
            Principal::management_canister(),
            "get_ecdsa_public_key", // WARNING! This method renamed in dfx 0.10.0 to `ecdsa_public_key`
            (request,),
            ECDSAPublicKeyResponse
        )
        .await
        .map(|res| Pubkey::new(res.public_key))
    }

    pub async fn sign_with_ecdsa(
        hash: Vec<u8>,
        derivation_path: Vec<Vec<u8>>,
    ) -> Result<SignWithECDSAReply, (RejectionCode, String)> {
        let request = SignWithECDSAArgs {
            key_id: "secp256k1".into(),
            message_hash: hash,
            derivation_path,
        };
        virtual_canister_call!(
            Principal::management_canister(),
            "sign_with_ecdsa",
            (request,),
            ic_ic00_types::SignWithECDSAReply
        )
        .await
    }

    pub async fn sign_canister_request<T>(
        canister: Principal,
        method_name: &str,
        args: T,
        pk: &Pubkey,
    ) -> Result<CallSignature, String>
    where
        T: ArgumentEncoder,
    {
        let der_pubkey = der_encode_pub_key(pk);

        let sender = Principal::self_authenticating(&der_pubkey);
        let args = encode_args(args).map_err(|err| format!("{err:?}"))?;
        let ingress_expiry_sec = ic_cdk::api::time() / 1_000_000_000 + 5 * 60;
        let ingress_expiry_nano = ingress_expiry_sec * 1_000_000_000;
        let request = update_content(
            sender,
            &canister,
            method_name,
            &args,
            ingress_expiry_nano.to_le_bytes().to_vec(), // nonce
            ingress_expiry_nano,
        );

        let request_id = to_request_id(&request).expect("request id err");
        let msg = construct_message(&request_id);
        let mut hasher = sha2::Sha256::new();
        hasher.update(&msg);

        let res = Self::sign_with_ecdsa(hasher.finalize().to_vec(), vec![])
            .await
            .map_err(|(_, err)| err)?;

        let envelope = Envelope {
            content: request,
            sender_pubkey: Some(der_pubkey.clone()),
            sender_sig: Some(res.signature),
        };

        let mut serialized_bytes = Vec::new();
        let mut serializer = serde_cbor::Serializer::new(&mut serialized_bytes);
        serializer.self_describe().expect("ser err");
        envelope.serialize(&mut serializer).expect("serialize err");
        let content = serialized_bytes;

        let paths: Vec<Vec<Label>> =
            vec![vec!["request_status".into(), request_id.as_slice().into()]];
        let request_new = read_state_content(sender, paths, ingress_expiry_nano);
        let request_id_new = to_request_id(&request_new).expect("request id error");
        let msg = construct_message(&request_id_new);
        let mut hasher = sha2::Sha256::new();
        hasher.update(&msg);

        let res = Self::sign_with_ecdsa(hasher.finalize().to_vec(), vec![])
            .await
            .map_err(|(_, err)| err)?;

        let envelope = Envelope {
            content: request_new,
            sender_pubkey: Some(der_pubkey),
            sender_sig: Some(res.signature),
        };
        let mut serialized_bytes = Vec::new();
        let mut serializer = serde_cbor::Serializer::new(&mut serialized_bytes);
        serializer.self_describe().expect("ser err");
        envelope.serialize(&mut serializer).expect("serialize err");
        let status_request_content = serialized_bytes;

        Ok(CallSignature {
            sender,
            recipient: canister,
            request_id: request_id.to_vec(),
            content: content.to_vec(),
            status_request_content,
        })
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
            oid: ObjectIdentifier::new("1.2.840.10045.2.1")
                .expect("never fails as it is Secp256 oid"),
            parameters: Some((&ObjectIdentifier::new("1.3.132.0.10").expect("never fails")).into()),
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
