use std::mem;
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use candid::Principal;
use ic_cdk::api::management_canister::provisional::CanisterId;
use pocket_ic::common::rest::{BlobCompression, BlobId};
use pocket_ic::{CallError, PocketIc, UserError, WasmResult};

use super::create_pocket_ic_client;

/// Client which performs blocking IO from PocketIc inside a tokio blocking tasks.
#[derive(Clone)]
pub struct PocketIcAsync(Arc<PocketIcAsyncClient>);

impl PocketIcAsync {
    /// Creates a new client.
    /// The server is started if it's not already running.
    ///
    /// If server is not installed, PocketIcAsync::init() should be used instead.
    pub async fn new() -> Self {
        let client = tokio::task::spawn_blocking(create_pocket_ic_client)
            .await
            .unwrap();
        Self(Arc::new(PocketIcAsyncClient::new(client)))
    }

    /// Creates a new client.
    /// Install and run server if needed.
    ///
    /// See [[super::init_pocket_ic]] for more information.
    pub async fn init() -> Self {
        let client = tokio::task::spawn_blocking(super::init_pocket_ic)
            .await
            .unwrap();
        Self(Arc::new(PocketIcAsyncClient::new(client)))
    }

    /// Upload and store a binary blob to the PocketIC server.
    pub async fn upload_blob(&self, blob: Vec<u8>, compression: BlobCompression) -> BlobId {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.upload_blob(blob, compression))
            .await
            .unwrap()
    }

    /// Set stable memory of a canister. Optional GZIP compression can be used for reduced
    /// data traffic.
    pub async fn set_stable_memory(
        &self,
        canister_id: Principal,
        data: Vec<u8>,
        compression: BlobCompression,
    ) {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || {
            client.set_stable_memory(canister_id, data, compression)
        })
        .await
        .unwrap()
    }

    /// Get stable memory of a canister.
    pub async fn get_stable_memory(&self, canister_id: Principal) -> Vec<u8> {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.get_stable_memory(canister_id))
            .await
            .unwrap()
    }

    /// List all instances and their status.
    pub async fn list_instances() -> Vec<String> {
        tokio::task::spawn_blocking(PocketIc::list_instances)
            .await
            .unwrap()
    }

    // Verify a canister signature.
    pub async fn verify_canister_signature(
        &self,
        msg: Vec<u8>,
        sig: Vec<u8>,
        pubkey: Vec<u8>,
        root_pubkey: Vec<u8>,
    ) -> Result<(), String> {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || {
            client.verify_canister_signature(msg, sig, pubkey, root_pubkey)
        })
        .await
        .unwrap()
    }

    /// Make the IC produce and progress by one block.
    pub async fn tick(&self) {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.tick())
            .await
            .unwrap()
    }

    /// Get the root key of this IC instance
    pub async fn root_key(&self) -> Option<Vec<u8>> {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.root_key())
            .await
            .unwrap()
    }

    /// Get the current time of the IC.
    pub async fn get_time(&self) -> SystemTime {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.get_time())
            .await
            .unwrap()
    }

    /// Set the current time of the IC.
    pub async fn set_time(&self, time: SystemTime) {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.set_time(time))
            .await
            .unwrap()
    }

    /// Advance the time on the IC by some nanoseconds.
    pub async fn advance_time(&self, duration: Duration) {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.advance_time(duration))
            .await
            .unwrap()
    }

    /// Get the current cycles balance of a canister.
    pub async fn cycle_balance(&self, canister_id: Principal) -> u128 {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.cycle_balance(canister_id))
            .await
            .unwrap()
    }

    /// Add cycles to a canister. Returns the new balance.
    pub async fn add_cycles(&self, canister_id: Principal, amount: u128) -> u128 {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.add_cycles(canister_id, amount))
            .await
            .unwrap()
    }

    /// Execute an update call on a canister.
    pub async fn update_call(
        &self,
        canister_id: Principal,
        sender: Principal,
        method: String,
        payload: Vec<u8>,
    ) -> Result<WasmResult, UserError> {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || {
            client.update_call(canister_id, sender, &method, payload)
        })
        .await
        .unwrap()
    }

    /// Execute a query call on a canister.
    pub async fn query_call(
        &self,
        canister_id: Principal,
        sender: Principal,
        method: String,
        payload: Vec<u8>,
    ) -> Result<WasmResult, UserError> {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || {
            client.query_call(canister_id, sender, &method, payload)
        })
        .await
        .unwrap()
    }

    /// Create a canister with default settings.
    pub async fn create_canister(&self, sender: Option<Principal>) -> CanisterId {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.create_canister_with_settings(sender, None))
            .await
            .unwrap()
    }

    /// Install a WASM module on an existing canister.
    pub async fn install_canister(
        &self,
        canister_id: CanisterId,
        wasm_module: Vec<u8>,
        arg: Vec<u8>,
        sender: Option<Principal>,
    ) {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || {
            client.install_canister(canister_id, wasm_module, arg, sender)
        })
        .await
        .unwrap()
    }

    /// Upgrade a canister with a new WASM module.
    pub async fn upgrade_canister(
        &self,
        canister_id: CanisterId,
        wasm_module: Vec<u8>,
        arg: Vec<u8>,
        sender: Option<Principal>,
    ) -> Result<(), CallError> {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || {
            client.upgrade_canister(canister_id, wasm_module, arg, sender)
        })
        .await
        .unwrap()
    }

    /// Reinstall a canister WASM module.
    pub async fn reinstall_canister(
        &self,
        canister_id: CanisterId,
        wasm_module: Vec<u8>,
        arg: Vec<u8>,
        sender: Option<Principal>,
    ) -> Result<(), CallError> {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || {
            client.reinstall_canister(canister_id, wasm_module, arg, sender)
        })
        .await
        .unwrap()
    }

    /// Start a canister.
    pub async fn start_canister(
        &self,
        canister_id: CanisterId,
        sender: Option<Principal>,
    ) -> Result<(), CallError> {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.start_canister(canister_id, sender))
            .await
            .unwrap()
    }

    /// Stop a canister.
    pub async fn stop_canister(
        &self,
        canister_id: CanisterId,
        sender: Option<Principal>,
    ) -> Result<(), CallError> {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.stop_canister(canister_id, sender))
            .await
            .unwrap()
    }

    /// Delete a canister.
    pub async fn delete_canister(
        &self,
        canister_id: CanisterId,
        sender: Option<Principal>,
    ) -> Result<(), CallError> {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.delete_canister(canister_id, sender))
            .await
            .unwrap()
    }

    /// Checks whether the provided canister exists.
    pub async fn canister_exists(&self, canister_id: CanisterId) -> bool {
        let client = self.0.clone();
        tokio::task::spawn_blocking(move || client.canister_exists(canister_id))
            .await
            .unwrap()
    }
}

struct PocketIcAsyncClient(Option<PocketIc>);

impl PocketIcAsyncClient {
    pub fn new(inner: PocketIc) -> Self {
        Self(Some(inner))
    }
}

impl Deref for PocketIcAsyncClient {
    type Target = PocketIc;

    fn deref(&self) -> &Self::Target {
        // The `self.0` become `None`` only in the `Drop` impl.
        self.0.as_ref().unwrap()
    }
}

/// This implementation needed to perform blocking PocketIc::drop() operation in
/// the `tokio::task::spawn_blocking`` blocking context.
impl Drop for PocketIcAsyncClient {
    fn drop(&mut self) {
        let client = Option::take(&mut self.0);
        tokio::task::spawn_blocking(move || mem::drop(client));
    }
}

#[cfg(test)]
mod tests {
    use super::PocketIcAsync;

    #[tokio::test]
    async fn should_initialize_pocket_ic_async() {
        PocketIcAsync::init().await;
    }
}
