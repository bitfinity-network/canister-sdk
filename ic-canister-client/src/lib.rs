#[cfg(feature = "ic-agent-client")]
pub mod agent;

pub mod client;
pub mod error;
pub mod ic_client;

#[cfg(feature = "ic-agent-client")]
pub use agent::{AgentError, IcAgentClient};
#[cfg(feature = "ic-agent-client")]
pub use ic_agent;
pub use client::CanisterClient;
pub use error::{CanisterClientError, CanisterClientResult, IcError, IcResult};
pub use ic_client::IcCanisterClient;
