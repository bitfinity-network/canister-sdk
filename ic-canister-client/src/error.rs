use ic_exports::ic_cdk::api::call::RejectionCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CanisterClientError {
    #[error("canister call failed: {0:?}")]
    CanisterError(IcError),

    #[error(transparent)]
    CandidError(#[from] candid::Error),

    #[cfg(feature = "ic-agent-client")]
    #[error("ic agent error: {0}")]
    IcAgentError(#[from] ic_agent::agent::AgentError),

    #[cfg(feature = "state-machine-tests-client")]
    #[error("state machine test error: {0}")]
    StateMachineTestError(ic_exports::ic_test_state_machine::UserError),

    #[cfg(feature = "pocket-ic-client")]
    #[error("pocket-ic test error: {0}")]
    PocketIcTestError(pocket_ic::UserError),
}

impl From<pocket_ic::UserError> for CanisterClientError {
    fn from(error: pocket_ic::UserError) -> Self {
        CanisterClientError::PocketIcTestError(error)
    }
}

#[cfg(feature = "state-machine-tests-client")]
impl From<ic_exports::ic_test_state_machine::UserError> for CanisterClientError {
    fn from(error: ic_exports::ic_test_state_machine::UserError) -> Self {
        CanisterClientError::StateMachineTestError(error)
    }
}

pub type CanisterClientResult<T> = Result<T, CanisterClientError>;

/// This tuple is returned incase of IC errors such as Network, canister error.
pub type IcError = (RejectionCode, String);

/// This is the result type for all IC calls.
pub type IcResult<R> = Result<R, IcError>;
