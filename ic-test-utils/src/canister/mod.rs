//! Interact with canisters in tests.
//!
//! ```
//! use ic_test_utils::canister::Canister;
//!
//! # async fn run<'a, T>(canister: Canister<'a, T>, principal: candid::Principal, agent: &'a ic_agent::Agent) {
//! let wallet = Canister::new_wallet(agent, "bob").unwrap();
//! let management = Canister::new_management(agent);
//! # }
//! ```
use std::marker::PhantomData;

use candid::{CandidType, Encode, Principal};
use ic_agent::agent::{Agent, QueryBuilder, UpdateBuilder};

use crate::Result;

mod management;
mod wallet;

pub use management::Management;
pub use wallet::Wallet;

/// Type alias for the management canister
pub type ManagementCanister<'agent> = Canister<'agent, Management>;

/// Type alias for the wallet canister
pub type WalletCanister<'agent> = Canister<'agent, Wallet>;

/// Represent a Canister in a test case
pub struct Canister<'agent, T> {
    id: Principal,
    pub(crate) agent: &'agent Agent,
    _phantom_data: PhantomData<T>,
}

impl<'agent, T> Canister<'agent, T> {
    /// Create a new canister with a given `Principal`
    pub fn new(id: Principal, agent: &'agent Agent) -> Self {
        Self {
            id,
            agent,
            _phantom_data: PhantomData,
        }
    }

    /// The id of the canister
    pub fn principal(&self) -> &Principal {
        &self.id
    }

    /// Update call to the canister
    pub fn update<A: CandidType>(
        &self,
        method_name: impl Into<String>,
        args: Option<A>,
    ) -> Result<UpdateBuilder<'_>> {
        let mut builder = self.agent.update(&self.id, method_name);
        if let Some(ref args) = args {
            let args = Encode!(args)?;
            builder = builder.with_arg(args);
        }
        Ok(builder)
    }

    /// Query the canister
    pub fn query(&self, method_name: impl Into<String>) -> QueryBuilder<'_> {
        self.agent.query(&self.id, method_name)
    }
}
