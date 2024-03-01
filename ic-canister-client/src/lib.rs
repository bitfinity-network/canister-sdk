#[cfg(feature = "ic-agent-client")]
pub mod agent;

pub mod client;
pub mod error;
pub mod ic_client;

#[cfg(feature = "state-machine-tests-client")]
pub mod state_machine_tests;

#[cfg(feature = "pocket-ic-client")]
pub mod pocket_ic;

#[cfg(feature = "ic-agent-client")]
pub use agent::{AgentError, IcAgentClient};
pub use client::CanisterClient;
pub use error::{CanisterClientError, CanisterClientResult, IcError, IcResult};
#[cfg(feature = "ic-agent-client")]
pub use ic_agent;
pub use ic_client::IcCanisterClient;
#[cfg(feature = "pocket-ic-client")]
pub use pocket_ic::PocketIcClient;
#[cfg(feature = "state-machine-tests-client")]
pub use state_machine_tests::StateMachineCanisterClient;
