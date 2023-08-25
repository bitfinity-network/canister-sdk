use std::borrow::Cow;

use candid::utils::ArgumentEncoder;
use candid::{encode_args, CandidType, Deserialize, Encode, Principal};

use super::{Agent, Canister};
use crate::Result;

/// The install mode of the canister to install. If a canister is already installed,
/// using [InstallMode::Install] will be an error. [InstallMode::Reinstall] overwrites
/// the module, and [InstallMode::Upgrade] performs an Upgrade step.
#[derive(Copy, Clone, CandidType, Deserialize, Eq, PartialEq)]
pub enum InstallMode {
    /// Install wasm
    #[serde(rename = "install")]
    Install,
    /// Reinstall wasm
    #[serde(rename = "reinstall")]
    Reinstall,
    /// Upgrade wasm
    #[serde(rename = "upgrade")]
    Upgrade,
}

/// Installation arguments for [`Canister::install_code`].
#[derive(CandidType, Deserialize)]
pub struct CanisterInstall<'a> {
    /// [`InstallMode`]
    pub mode: InstallMode,
    /// Canister id
    pub canister_id: Principal,
    /// Wasm module as raw bytes
    #[serde(with = "serde_bytes")]
    #[serde(borrow)]
    pub wasm_module: Cow<'a, [u8]>,
    #[serde(with = "serde_bytes")]
    /// Any aditional arguments to be passed along
    pub arg: Vec<u8>,
}

#[derive(CandidType, Deserialize)]
struct In {
    canister_id: Principal,
}

// -----------------------------------------------------------------------------
//     - Management container -
// -----------------------------------------------------------------------------

/// The management canister is used to install code, upgrade, stop and delete
/// canisters.
///
/// ```
/// # use ic_agent::Agent;
/// use ic_test_utils::canister::Canister;
/// # async fn run(agent: &Agent, principal: candid::Principal) {
/// let management = Canister::new_management(agent);
/// management.stop_canister(&agent, principal).await;
/// # }
/// ```
pub struct Management;

impl<'agent> Canister<'agent, Management> {
    /// Create a new management canister
    pub fn new_management(agent: &'agent Agent) -> Self {
        let id = Principal::management_canister();
        Self::new(id, agent)
    }

    async fn _install_code<T: ArgumentEncoder>(
        &self,
        agent: &Agent,
        canister_id: Principal,
        bytecode: Cow<'_, [u8]>,
        mode: InstallMode,
        arg: T,
    ) -> Result<()> {
        let install_args = CanisterInstall {
            mode,
            canister_id,
            wasm_module: bytecode,
            arg: encode_args(arg)?,
        };

        let args = Encode!(&install_args)?;
        agent
            .update(&Principal::management_canister(), "install_code")
            .with_arg(args)
            .call_and_wait()
            .await?;

        Ok(())
    }

    /// Install code in an existing canister.
    /// To create a canister first use [`Canister::create_canister`]
    pub async fn install_code<T: ArgumentEncoder>(
        &self,
        agent: &Agent,
        canister_id: Principal,
        bytecode: Cow<'_, [u8]>,
        arg: T,
    ) -> Result<()> {
        self._install_code(agent, canister_id, bytecode, InstallMode::Install, arg)
            .await
    }

    /// Replaces code of an existing canister. This method completely erases the old canister with
    /// all its state. If you want to upgrade the canister, call [`Canister::upgrade_code`] instead.
    pub async fn reinstall_code<T: ArgumentEncoder>(
        &self,
        agent: &Agent,
        canister_id: Principal,
        bytecode: Cow<'_, [u8]>,
        arg: T,
    ) -> Result<()> {
        self._install_code(agent, canister_id, bytecode, InstallMode::Reinstall, arg)
            .await
    }

    /// Upgrade an existing canister.
    /// Upgrading a canister for a test is possible even if the underlying binary hasn't changed
    pub async fn upgrade_code<T: ArgumentEncoder>(
        &self,
        agent: &Agent,
        canister_id: Principal,
        bytecode: Cow<'_, [u8]>,
        arg: T,
    ) -> Result<()> {
        self._install_code(agent, canister_id, bytecode, InstallMode::Upgrade, arg)
            .await
    }

    /// Stop a running canister
    pub async fn stop_canister(
        &self,
        agent: &Agent,
        canister_id: Principal, // canister to stop
    ) -> Result<()> {
        let arg = Encode!(&In { canister_id })?;
        agent
            .update(&Principal::management_canister(), "stop_canister")
            .with_arg(arg)
            .call_and_wait()
            .await?;
        Ok(())
    }

    /// Delete a canister. The target canister can not be running,
    /// make sure the canister has stopped first: [`Canister::stop_canister`]
    pub async fn delete_canister(
        &self,
        agent: &Agent,
        canister_id: Principal, // canister to delete
    ) -> Result<()> {
        let arg = Encode!(&In { canister_id })?;
        agent
            .update(&Principal::management_canister(), "delete_canister")
            .with_arg(arg)
            .call_and_wait()
            .await?;
        Ok(())
    }
}
