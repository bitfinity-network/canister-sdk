use std::sync::Arc;

use candid::utils::ArgumentEncoder;
use candid::{CandidType, Decode, Principal};
use ic_exports::ic_kit::RejectionCode;
use ic_exports::ic_test_state_machine::{StateMachine, WasmResult};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::{CanisterClient, CanisterClientError, CanisterClientResult};

/// A client for interacting with a canister inside dfinity's
/// state machine tests framework.
#[derive(Clone)]
pub struct StateMachineCanisterClient {
    state_machine: Arc<Mutex<StateMachine>>,
    canister: Principal,
    caller: Principal,
}

impl StateMachineCanisterClient {
    /// Creates a new instance of a StateMachineCanisterClient.
    pub fn new(
        state_machine: Arc<Mutex<StateMachine>>,
        canister: Principal,
        caller: Principal,
    ) -> Self {
        Self {
            state_machine,
            canister,
            caller,
        }
    }

    /// Returns the caller of the canister.
    pub fn caller(&self) -> Principal {
        self.caller
    }

    /// Replace the caller.
    pub fn set_caller(&mut self, caller: Principal) {
        self.caller = caller;
    }

    /// Returns the canister of the canister.
    pub fn canister(&self) -> Principal {
        self.canister
    }

    /// Replace the canister to call.
    pub fn set_canister(&mut self, canister: Principal) {
        self.canister = canister;
    }

    /// Returns the state machine of the canister.
    pub fn state_machine(&self) -> &Mutex<StateMachine> {
        self.state_machine.as_ref()
    }

    /// Performs a blocking action with state machine and awaits the result.
    /// 
    /// Arguments of the closure `f`:
    /// 1) `env` - The state machine environment.
    /// 2) `canister` - The canister principal.
    /// 3) `caller` - The caller principal.
    pub async fn with_state_machine<F, R>(&self, f: F) -> R
    where
        F: Send + FnOnce(&StateMachine, Principal, Principal) -> R + 'static,
        R: Send + 'static,
    {
        let client = self.state_machine.clone();
        let cansiter = self.canister;
        let caller = self.caller;

        let result = tokio::task::spawn_blocking(move || {
            let locked_client = client.blocking_lock();
            f(&locked_client, cansiter, caller)
        })
        .await
        .unwrap();

        result
    }
}

#[async_trait::async_trait]
impl CanisterClient for StateMachineCanisterClient {
    async fn update<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send,
        R: for<'de> Deserialize<'de> + CandidType,
    {
        let args = candid::encode_args(args)?;
        let method = String::from(method);

        let call_result = self
            .with_state_machine(move |env, canister, caller| env.update_call(canister, caller, &method, args))
            .await?;

        let reply = match call_result {
            WasmResult::Reply(reply) => reply,
            WasmResult::Reject(e) => {
                return Err(CanisterClientError::CanisterError((
                    RejectionCode::CanisterError,
                    e,
                )));
            }
        };

        let decoded = Decode!(&reply, R)?;
        Ok(decoded)
    }

    async fn query<T, R>(&self, method: &str, args: T) -> CanisterClientResult<R>
    where
        T: ArgumentEncoder + Send,
        R: for<'de> Deserialize<'de> + CandidType,
    {
        let args = candid::encode_args(args)?;
        let method = String::from(method);

        let call_result = self
            .with_state_machine(move |env, canister, caller| env.query_call(canister, caller, &method, args))
            .await?;

        let reply = match call_result {
            WasmResult::Reply(reply) => reply,
            WasmResult::Reject(e) => {
                return Err(CanisterClientError::CanisterError((
                    RejectionCode::CanisterError,
                    e,
                )));
            }
        };

        let decoded = Decode!(&reply, R)?;
        Ok(decoded)
    }
}
