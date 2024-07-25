use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use ic_canister::{
    generate_exports, generate_idl, query, state_getter, update, Canister, Idl, PreUpdate,
};
use ic_exports::ic_kit::ic;

pub use crate::canister::state::LogState;
use crate::did::{LogCanisterError, LogCanisterSettings, LoggerPermission, Pagination};
use crate::writer::Logs;

pub mod inspect;
mod state;

/// Canister trait that provides common method for configuring and using canister logger.
///
/// Check out the `log_canister` example in the `examples` directory for a guide on how to add these
/// methods to you canister. In short, to use this implementation of the logger, you need to:
///
/// * implement `LogCanister` trait for your type
/// * call [`LogState::init`] method from the `#[init]` method of your canister.
/// * call [`inspect::logger_canister_inspect`] function from the `#[inspect_message]` method of
///   your canister.
///
/// # Permissions
///
/// Most operations in the `LogCanister` require the caller to have [`LoggerPermission`]s assigned
/// to them.
///
/// * `Read` permission allows a principal to get the logs with `ic_logs` method.
/// * `Configure` permission allows changing the logger configuration and manager logger permissions.
///   If a principal has `Configure` permission, `Read` permission is also assumed for that
///   principal.
///
/// # Configuration and ways to get logs
///
/// There are two ways to get the logs from the logger canister:
///
/// 1. Using IC management canister `get_canister_logs` method. To make the canister write logs
///    with the IC API, [`LogCanisterSettings::enable_console`] must be set to `true` (it
///    is enabled by default, so if `None` is given at the canister initialization, it will also
///    be considered as `true`). The logs written by this method are not affected by other
///    settings, such as number of in-memory logs and max log entry length (but they do apply the
///    logging filter). Also, they use native IC approach for checking permissions to get the logs
///    (it can be configured to allow access to the logs only to the canister controllers or to
///    anyone using the canister settings). This method can be used to get logs from trapped
///    operations.
///
/// 2. Using canister `ic_logs` method. Logs returned by this method are stored in the canister
///    memory. To limit the size of the memory that can be dedicated to the logs, configure
///    max number of entries to store and max size of a single entry. This method cannot store
///    logs from operations that trapped, and the logs storage is reset when the canister is
///    upgraded.
pub trait LogCanister: Canister + PreUpdate {
    /// State of the logger. Usually the implementation of this method would look like:
    ///
    /// ```ignore
    /// use ic_storage::IcStorage;
    /// fn log_state(&self) -> Rc<RefCell<LogState>> {
    ///     LogState::get()
    /// }
    /// ```
    #[state_getter]
    fn log_state(&self) -> Rc<RefCell<LogState>>;

    /// Returns canister logs.
    ///
    /// To use this method the caller must have [`LoggerPermission::Read`] permission.
    ///
    /// `pagination.offset` value specifies an absolute identifier of the first log entry to be
    /// returned. If the given offset is larger than the max id of the logs in the canister,
    /// an empty response will be returned.
    ///
    /// To get the maximum identifier of the logs currently stored in the canister, this method
    /// can be used with `pagination.count == 0`.
    ///
    /// # Traps
    ///
    /// Traps if the caller does not have [`LoggerPermission::Read`] permission.
    #[query(trait = true)]
    fn ic_logs(&self, pagination: Pagination) -> Logs {
        self.log_state()
            .borrow()
            .get_logs(ic::caller(), pagination)
            .expect("failed to get logs")
    }

    /// Sets the logger filter string.
    ///
    /// To call this method, the caller must have [`LoggerPermission::Configure`] permission.
    ///
    /// To turn off logging for the canister, use `filter == "off"`.
    ///
    /// # Errors
    ///
    /// * [`LogCanisterError::InvalidConfiguration`] if the `filter` string is invalid.
    ///
    /// # Traps
    ///
    /// Traps if the caller doesn't have [`LoggerPermission::Configure`] permission of if the
    /// logger state is not initialized.
    #[update(trait = true)]
    fn set_logger_filter(&mut self, filter: String) -> Result<(), LogCanisterError> {
        // This method returns a `Result` to emphasize that the operation can fail if the
        // argument is incorrect. But we still want to panic on failed permission check to make
        // the API work same with or without "inspect_message" check.
        match self
            .log_state()
            .borrow_mut()
            .set_logger_filter(ic::caller(), filter)
        {
            // We want to return an error variant in case the filter string is not valid
            err @ Err(LogCanisterError::InvalidConfiguration(_)) => err,
            result => {
                result.expect("failed to update configuration");
                Ok(())
            }
        }
    }

    /// Updates the maximum number of log entries stored in the canister memory.
    ///
    /// To call this method, the caller must have [`LoggerPermission::Configure`] permission.
    ///
    /// # Traps
    ///
    /// Traps if the caller doesn't have [`LoggerPermission::Configure`] permission of if the
    /// logger state is not initialized.
    #[update(trait = true)]
    fn set_logger_in_memory_records(&mut self, max_log_count: usize) {
        self.log_state()
            .borrow_mut()
            .set_in_memory_records(ic::caller(), max_log_count)
            .expect("failed to update configuration");
    }

    /// Returns the current logger settings.
    #[query(trait = true)]
    fn get_logger_settings(&self) -> LogCanisterSettings {
        self.log_state().borrow().get_settings().clone()
    }

    /// Add the given `permission` to the `to` principal.
    ///
    /// To call this method, the caller must have [`LoggerPermission::Configure`] permission.
    ///
    /// # Traps
    ///
    /// Traps if the caller doesn't have [`LoggerPermission::Configure`] permission of if the
    /// logger state is not initialized.
    #[update(trait = true)]
    fn add_logger_permission(&mut self, to: Principal, permission: LoggerPermission) {
        self.log_state()
            .borrow_mut()
            .add_permission(ic::caller(), to, permission)
            .expect("failed to add logger permission");
    }

    /// Remove the given `permission` from the `from` principal.
    ///
    /// To call this method, the caller must have [`LoggerPermission::Configure`] permission.
    ///
    /// # Traps
    ///
    /// Traps if the caller doesn't have [`LoggerPermission::Configure`] permission of if the
    /// logger state is not initialized.
    #[update(trait = true)]
    fn remove_logger_permission(&mut self, from: Principal, permission: LoggerPermission) {
        self.log_state()
            .borrow_mut()
            .remove_permission(ic::caller(), from, permission)
            .expect("failed to remove logger permission");
    }

    /// Return idl of the logger canister.
    fn get_idl() -> Idl {
        generate_idl!()
    }
}

generate_exports!(LogCanister);

#[cfg(test)]
mod tests {
    use super::*;

    struct LogTestImpl {}
    impl Canister for LogTestImpl {
        fn init_instance() -> Self {
            todo!()
        }

        fn from_principal(_principal: Principal) -> Self {
            todo!()
        }

        fn principal(&self) -> Principal {
            todo!()
        }
    }

    impl PreUpdate for LogTestImpl {}
    impl LogCanister for LogTestImpl {
        fn log_state(&self) -> Rc<RefCell<LogState>> {
            todo!()
        }
    }

    #[test]
    fn generates_idl() {
        let idl = LogTestImpl::get_idl();
        assert!(!format!("{idl}").is_empty())
    }
}
