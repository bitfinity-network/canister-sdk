use ic_exports::ic_cdk::api;
use ic_exports::ic_kit::ic;
use ic_storage::IcStorage;

use crate::canister::LogState;
use crate::did::LoggerPermission;

/// Implementation of canister inspect logic for logger canister. Call this method from the
/// `#[inspect_message]` function of your canister.
///
/// # Traps
///
/// Traps with a corresponding method if logger permission check is not passed.
pub fn logger_canister_inspect() {
    let method = api::call::method_name();
    let state = LogState::get();
    let state = state.borrow();
    let caller = ic::caller();

    match method.as_str() {
        "ic_logs" => state.check_permission(caller, LoggerPermission::Read),
        "set_logger_filter"
        | "set_logger_in_memory_records"
        | "add_logger_permission"
        | "remove_logger_permission" => state.check_permission(caller, LoggerPermission::Configure),
        _ => Ok(()),
    }
    .expect("inspect check failed");
}
