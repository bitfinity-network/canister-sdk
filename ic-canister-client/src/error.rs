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

    #[cfg(feature = "pocket-ic-client")]
    #[error("pocket-ic test error: {0:?}")]
    PocketIcTestError(ic_exports::pocket_ic::ErrorCode),
}

#[cfg(feature = "pocket-ic-client")]
impl From<ic_exports::pocket_ic::ErrorCode> for CanisterClientError {
    fn from(error: ic_exports::pocket_ic::ErrorCode) -> Self {
        CanisterClientError::PocketIcTestError(error)
    }
}

#[cfg(feature = "pocket-ic-client")]
impl From<ic_exports::pocket_ic::RejectResponse> for CanisterClientError {
    fn from(response: ic_exports::pocket_ic::RejectResponse) -> Self {
        CanisterClientError::PocketIcTestError(response.error_code)
    }
}

pub type CanisterClientResult<T> = Result<T, CanisterClientError>;

/// This tuple is returned incase of IC errors such as Network, canister error.
pub type IcError = (RejectionCode, String);

/// This is the result type for all IC calls.
pub type IcResult<R> = Result<R, IcError>;
