use ic_exports::ic_cdk::api;
use ic_exports::ic_kit::ic;
use ic_storage::IcStorage;

use crate::canister::LogState;
use crate::did::LoggerPermission;

pub fn logger_canister_inspect() {
    let method = api::call::method_name();
    let state = LogState::get().borrow();
    let caller = ic::caller();

    match method.as_str() {
        "ic_logs" => state.check_permissions(caller, LoggerPermission::Read),
        "set_logger_filter"
        | "set_logger_in_memory_records"
        | "add_logger_permission"
        | "remove_logger_permission" => {
            state.check_permissions(caller, LoggerPermission::Configure)
        }
        _ => {}
    }
}
