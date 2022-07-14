use crate::error::FactoryError;
use candid::utils::ArgumentEncoder;
use candid::Principal;
use ic_cdk::api::call::CallResult;
use ic_helpers::management::{Canister as ManagementCanister, CanisterSettings, InstallCodeMode};

pub async fn create_canister<T: ArgumentEncoder>(
    wasm_module: Vec<u8>,
    init_args: T,
    cycles: u64,
    controllers: Option<Vec<Principal>>,
) -> CallResult<Principal> {
    let settings = CanisterSettings {
        controllers,
        compute_allocation: None,
        memory_allocation: None,
        freezing_threshold: None,
    };

    let canister = ManagementCanister::create(Some(settings), cycles).await?;
    canister
        .install_code(InstallCodeMode::Install, wasm_module, init_args)
        .await?;

    Ok(canister.into())
}

pub async fn upgrade_canister(canister_id: Principal, wasm_module: Vec<u8>) -> CallResult<()> {
    ManagementCanister::from(canister_id)
        .install_code(InstallCodeMode::Upgrade, wasm_module, ())
        .await
}

pub async fn drop_canister(canister: Principal) -> Result<(), FactoryError> {
    let canister = ic_helpers::management::Canister::from(canister);
    canister
        .stop()
        .await
        .map_err(|(_, e)| FactoryError::ManagementError(e))?;
    canister
        .delete()
        .await
        .map_err(|(_, e)| FactoryError::ManagementError(e))?;

    Ok(())
}
