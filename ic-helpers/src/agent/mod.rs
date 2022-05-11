use crate::agent::request_id::RequestId;
use candid::types::ic_types::hash_tree::Label;
use ic_cdk::export::Principal;
use serde::{Deserialize, Serialize};

pub mod error;
pub mod request_id;

#[cfg(feature = "agent")]
mod agent;
#[cfg(feature = "agent")]
pub use agent::*;

const IC_REQUEST_DOMAIN_SEPARATOR: &[u8; 11] = b"\x0Aic-request";

pub fn update_content(
    sender: Principal,
    canister_id: &Principal,
    method_name: &str,
    arg: &[u8],
    nonce: Vec<u8>,
    ingress_expiry: u64,
) -> CallRequestContent {
    CallRequestContent::CallRequest {
        canister_id: *canister_id,
        method_name: method_name.into(),
        arg: arg.to_vec(),
        nonce: Some(nonce),
        sender,
        ingress_expiry,
    }
}

pub fn construct_message(request_id: &RequestId) -> Vec<u8> {
    let mut buf = vec![];
    buf.extend_from_slice(IC_REQUEST_DOMAIN_SEPARATOR);
    buf.extend_from_slice(request_id.as_slice());
    buf
}

pub fn read_state_content(
    sender: Principal,
    paths: Vec<Vec<Label>>,
    ingress_expiry: u64,
) -> ReadStateContent {
    ReadStateContent::ReadStateRequest {
        sender,
        paths,
        ingress_expiry,
    }
}

// A request as submitted to /api/v2/.../read_state
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "request_type")]
pub enum ReadStateContent {
    #[serde(rename = "read_state")]
    ReadStateRequest {
        ingress_expiry: u64,
        sender: Principal,
        paths: Vec<Vec<Label>>,
    },
}

// A request as submitted to /api/v2/.../call
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "request_type")]
pub enum CallRequestContent {
    #[serde(rename = "call")]
    CallRequest {
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(with = "serde_bytes")]
        nonce: Option<Vec<u8>>,
        ingress_expiry: u64,
        sender: Principal,
        canister_id: Principal,
        method_name: String,
        #[serde(with = "serde_bytes")]
        arg: Vec<u8>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Envelope<T: Serialize> {
    pub content: T,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "serde_bytes")]
    pub sender_pubkey: Option<Vec<u8>>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "serde_bytes")]
    pub sender_sig: Option<Vec<u8>>,
}
