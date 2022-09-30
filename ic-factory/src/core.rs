use crate::error::FactoryError;
use ic_exports::ic_cdk::{
    api::call::CallResult,
    export::candid::{utils::ArgumentEncoder, Principal},
};
use ic_helpers::management::{CanisterSettings, InstallCodeMode, ManagementPrincipalExt};

pub async fn create_canister<T: ArgumentEncoder + Send>(
    wasm_module: Vec<u8>,
    init_args: T,
    cycles: u64,
    controllers: Option<Vec<Principal>>,
) -> CallResult<Principal> {
    let settings = CanisterSettings {
        controllers,
        ..Default::default()
    };

    let canister = <Principal as ManagementPrincipalExt>::create(Some(settings), cycles).await?;
    canister
        .install_code(InstallCodeMode::Install, wasm_module, init_args)
        .await?;

    Ok(canister)
}

pub async fn upgrade_canister(canister_id: Principal, wasm_module: Vec<u8>) -> CallResult<()> {
    canister_id
        .install_code(InstallCodeMode::Upgrade, wasm_module, ())
        .await
}

pub async fn drop_canister(canister: Principal) -> Result<(), FactoryError> {
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
