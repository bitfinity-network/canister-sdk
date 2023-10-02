use ic_exports::ic_cdk::api::call::RejectionCode;
#[cfg(feature = "state-machine-tests-client")]
use ic_exports::ic_test_state_machine::UserError;
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
    StateMachineTestError(UserError),
}

#[cfg(feature = "state-machine-tests-client")]
impl From<UserError> for CanisterClientError {
    fn from(error: UserError) -> Self {
        CanisterClientError::StateMachineTestError(error)
    }
}

pub type CanisterClientResult<T> = Result<T, CanisterClientError>;

/// This tuple is returned incase of IC errors such as Network, canister error.
pub type IcError = (RejectionCode, String);

/// This is the result type for all IC calls.
pub type IcResult<R> = Result<R, IcError>;
