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

mod state;

pub trait LogCanister: Canister + PreUpdate {
    #[state_getter]
    fn log_state(&self) -> Rc<RefCell<LogState>>;

    /// Gets the logs
    /// - `count` is the number of logs to return
    #[query(trait = true)]
    fn ic_logs(&self, page: Pagination) -> Logs {
        self.log_state()
            .borrow()
            .get_logs(ic::caller(), page)
            .expect("Failed to get logs.")
    }

    /// Updates the runtime configuration of the logger with a new filter in the same form as the `RUST_LOG`
    /// environment variable.
    /// Example of valid filters:
    /// - info
    /// - debug,crate1::mod1=error,crate1::mod2,crate2=debug
    #[update(trait = true)]
    fn set_logger_filter(&mut self, filter: String) -> Result<(), LogCanisterError> {
        self.log_state()
            .borrow_mut()
            .set_logger_filter(ic::caller(), filter)
    }

    #[update(trait = true)]
    fn set_logger_in_memory_records(
        &mut self,
        max_log_count: usize,
    ) -> Result<(), LogCanisterError> {
        self.log_state()
            .borrow_mut()
            .set_in_memory_records(ic::caller(), max_log_count)
    }

    #[query(trait = true)]
    fn get_logger_settings(&self) -> LogCanisterSettings {
        self.log_state().borrow().get_settings().clone().into()
    }

    #[update(trait = true)]
    fn add_logger_permission(&mut self, to: Principal, permission: LoggerPermission) {
        self.log_state()
            .borrow_mut()
            .add_permission(ic::caller(), to, permission)
            .expect("Failed to add logger permission");
    }

    #[update(trait = true)]
    fn remove_logger_permission(&mut self, from: Principal, permission: LoggerPermission) {
        self.log_state()
            .borrow_mut()
            .remove_permission(ic::caller(), from, permission)
            .expect("Failed to remove logger permission");
    }

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
