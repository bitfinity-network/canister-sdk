use std::borrow::Cow;
use std::cell::RefCell;

use candid::{Decode, Encode, Principal};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{Bound, CellStructure, StableCell, Storable, VirtualMemory};
use ic_storage::IcStorage;

use crate::did::{LogCanisterError, LogCanisterSettings, LoggerAcl, LoggerPermission, Pagination};
use crate::writer::{InMemoryWriter, Logs};
use crate::{take_memory_records, LogSettingsV2, LoggerConfig};

thread_local! {
    static LOGGER_CONFIG: RefCell<Option<LoggerConfig>> = const { RefCell::new(None) };
}

/// State of the logger canister.
///
/// Before logger can be used, it must be initialized with the [`LogState::init`] method.
#[derive(Default, IcStorage)]
pub struct LogState {
    settings: Option<StableCell<StorableLogSettings, VirtualMemory<DefaultMemoryImpl>>>,
}

impl LogState {
    /// Initializes the logger with the given settings.
    ///
    /// IMPORTANT: this method must be called only once during the runtime of the application. A
    /// canister process can be started with `#[init]` and `#[post_upgrade]` methods. In case of
    /// `post_upgrade`, use [`LogState::reload`] method instead.
    ///
    /// # Arguments
    /// * `caller` - caller of the `#[init]` method of the canister. This principal is used to
    ///   create a default ACL in case one is not given in the configuration.
    /// * `memory` - stable memory to use for the logger configuration.
    /// * `log_settings` - logger settings.
    ///
    /// # Errors
    ///
    /// Returns [`LogCanisterError::AlreadyInitialized`] if called more than once during the
    /// lifetime of the application.
    pub fn init(
        &mut self,
        caller: Principal,
        memory: VirtualMemory<DefaultMemoryImpl>,
        log_settings: LogCanisterSettings,
    ) -> Result<(), LogCanisterError> {
        if LOGGER_CONFIG.with(|logger_config| logger_config.borrow().is_some()) {
            return Err(LogCanisterError::AlreadyInitialized);
        }

        let acl = log_settings
            .acl
            .clone()
            .unwrap_or_else(|| [(caller, LoggerPermission::Configure)].into());
        let settings = log_settings.into();

        Self::init_log(&settings)?;

        self.settings = Some(
            StableCell::new(memory, StorableLogSettings(settings, acl))
                .map_err(|_| LogCanisterError::InvalidMemory)?,
        );

        // Print this out without using log in case the given parameters prevent logs to be printed.
        #[cfg(target_arch = "wasm32")]
        ic_exports::ic_kit::ic::print(format!(
            "Initialized logging with settings: {:?}",
            self.get_settings()
        ));

        Ok(())
    }

    /// Returns current settings of the logger.
    pub fn get_settings(&self) -> LogCanisterSettings {
        let StorableLogSettings(settings, acl) = self
            .settings
            .as_ref()
            .map(|v| v.get().clone())
            .unwrap_or_default();
        (settings, acl).into()
    }

    /// Set logger filter.
    pub fn set_logger_filter(
        &mut self,
        caller: Principal,
        filter_value: String,
    ) -> Result<(), LogCanisterError> {
        self.check_permission(caller, LoggerPermission::Configure)?;

        // This operation must be the first one as it is the only one that may return error.
        // It is not guaranteed that the caller of this function will revert the canister state
        // changes, so we must take care not to update the state if the filter is invalid.
        LOGGER_CONFIG.with(|config| {
            if let Some(config) = &mut *config.borrow_mut() {
                config.update_filters(&filter_value)
            } else {
                Err(LogCanisterError::NotInitialized)
            }
        })?;

        if let Some(cell) = &mut self.settings {
            let mut settings = cell.get().clone();
            settings.0.log_filter.clone_from(&filter_value);
            cell.set(settings)
                .expect("failed to write settings to stable memory");
        }

        log::info!("Updated log filter to: {filter_value:?}");

        Ok(())
    }

    /// Set `in_memory_records` settings.
    pub fn set_in_memory_records(
        &mut self,
        caller: Principal,
        count: usize,
    ) -> Result<(), LogCanisterError> {
        self.check_permission(caller, LoggerPermission::Configure)?;

        InMemoryWriter::change_capacity(count);

        if let Some(cell) = &mut self.settings {
            let mut settings = cell.get().clone();
            settings.0.in_memory_records = count;
            cell.set(settings)
                .expect("failed to write settings to stable memory");
        }

        Ok(())
    }

    /// Return the logs from the memory.
    pub fn get_logs(&self, caller: Principal, page: Pagination) -> Result<Logs, LogCanisterError> {
        self.check_permission(caller, LoggerPermission::Read)?;
        Ok(take_memory_records(page.count, page.offset))
    }

    /// Reloads the configuration of the logger from the stable memory and initializes the logger.
    ///
    /// This method should be called from `#[post_upgrade]` method.
    pub fn reload(
        &mut self,
        memory: VirtualMemory<DefaultMemoryImpl>,
    ) -> Result<(), LogCanisterError> {
        if LOGGER_CONFIG.with(|logger_config| logger_config.borrow().is_some()) {
            return Err(LogCanisterError::AlreadyInitialized);
        }

        self.settings = Some(
            StableCell::new(memory, StorableLogSettings::default())
                .map_err(|_| LogCanisterError::InvalidMemory)?,
        );
        let settings = self.settings.as_ref().unwrap().get().clone();

        if settings.1 == LoggerAcl::default() {
            return Err(LogCanisterError::InvalidMemory);
        }

        Self::init_log(&settings.0)?;

        Ok(())
    }

    /// Add permission for the `to` principal.
    pub fn add_permission(
        &mut self,
        caller: Principal,
        to: Principal,
        permission: LoggerPermission,
    ) -> Result<(), LogCanisterError> {
        self.check_permission(caller, LoggerPermission::Configure)?;

        if let Some(cell) = &mut self.settings {
            let mut settings = cell.get().clone();
            settings.1.insert((to, permission));
            cell.set(settings)
                .expect("failed to write settings to stable memory");
        }

        Ok(())
    }

    /// Remove permission from the `from` principal.
    pub fn remove_permission(
        &mut self,
        caller: Principal,
        from: Principal,
        permission: LoggerPermission,
    ) -> Result<(), LogCanisterError> {
        self.check_permission(caller, LoggerPermission::Configure)?;

        if let Some(cell) = &mut self.settings {
            let mut settings = cell.get().clone();
            settings.1.remove(&(from, permission));
            cell.set(settings)
                .expect("failed to write settings to stable memory");
        }

        Ok(())
    }

    pub fn acl(&self) -> LoggerAcl {
        self.settings
            .as_ref()
            .map(|cell| cell.get().clone())
            .unwrap_or_default()
            .1
    }

    fn init_log(log_settings: &LogSettingsV2) -> Result<(), LogCanisterError> {
        let logger_config = {
            cfg_if::cfg_if! {
                if #[cfg(test)] {
                    let (_, config) = crate::Builder::default().try_parse_filters(&log_settings.log_filter)?.build();
                    config
                } else {
                    crate::init_log(log_settings)?
                }
            }
        };

        LOGGER_CONFIG.with(|config| config.borrow_mut().replace(logger_config));
        Ok(())
    }

    pub(crate) fn check_permission(
        &self,
        caller: Principal,
        logger_permission: LoggerPermission,
    ) -> Result<(), LogCanisterError> {
        let Some(cell) = self.settings.as_ref() else {
            return Err(LogCanisterError::NotInitialized);
        };

        let settings = cell.get();
        let acl = &settings.1;

        let allowed = match logger_permission {
            LoggerPermission::Read => {
                acl.contains(&(caller, LoggerPermission::Read))
                    || (acl.contains(&(caller, LoggerPermission::Configure)))
            }
            LoggerPermission::Configure => acl.contains(&(caller, LoggerPermission::Configure)),
        };

        if allowed {
            Ok(())
        } else {
            Err(LogCanisterError::NotAuthorized)
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct StorableLogSettings(pub LogSettingsV2, pub LoggerAcl);

impl Storable for StorableLogSettings {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::from(Encode!(&(&self.0, &self.1)).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let (settings, acl) = Decode!(&bytes, (LogSettingsV2, LoggerAcl)).unwrap();
        Self(settings, acl)
    }

    const BOUND: Bound = Bound::Unbounded;
}

#[cfg(test)]
mod tests {
    use ic_stable_structures::{IcMemoryManager, MemoryId};

    use super::*;

    thread_local! {
        static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());
    }

    fn admin() -> Principal {
        Principal::from_slice(&[1; 20])
    }

    fn reader() -> Principal {
        Principal::from_slice(&[2; 20])
    }

    fn user() -> Principal {
        Principal::from_slice(&[5; 20])
    }

    fn test_memory() -> VirtualMemory<DefaultMemoryImpl> {
        MEMORY_MANAGER.with(|manager| manager.get(MemoryId::new(2)))
    }

    fn test_settings() -> LogSettingsV2 {
        LogSettingsV2 {
            enable_console: true,
            in_memory_records: 10,
            max_record_length: 1024,
            log_filter: "trace".to_string(),
        }
    }

    fn test_acl() -> LoggerAcl {
        [
            (admin(), LoggerPermission::Configure),
            (reader(), LoggerPermission::Read),
        ]
        .into()
    }

    fn test_canister_settings() -> LogCanisterSettings {
        (test_settings(), test_acl()).into()
    }

    fn test_state() -> LogState {
        let mut state = LogState::default();
        state
            .init(admin(), test_memory(), test_canister_settings())
            .unwrap();
        state
    }

    fn reset_config() {
        LOGGER_CONFIG.with(|v| {
            *v.borrow_mut() = None;
        })
    }

    #[test]
    fn init_stores_settings() {
        let mut state = LogState::default();
        let settings = LogSettingsV2 {
            enable_console: true,
            in_memory_records: 10,
            max_record_length: 1024,
            log_filter: "debug".to_string(),
        };
        state
            .init(
                admin(),
                test_memory(),
                (settings.clone(), test_acl()).into(),
            )
            .unwrap();

        assert_eq!(state.get_settings(), (settings, test_acl()).into());
    }

    #[test]
    fn init_configures_logger() {
        let _ = test_state();
        LOGGER_CONFIG.with(|config| {
            assert!(config.borrow().is_some(), "Config is not stored");
        })
    }

    #[test]
    fn init_fails_if_already_initialized() {
        let mut state = test_state();
        assert_eq!(
            state.init(admin(), test_memory(), test_canister_settings()),
            Err(LogCanisterError::AlreadyInitialized)
        );
    }

    #[test]
    fn init_fails_with_invalid_filter_string() {
        let mut state = LogState::default();
        assert_eq!(
            state.init(
                admin(),
                test_memory(),
                LogCanisterSettings {
                    log_filter: Some("crate=invalid".into()),
                    ..Default::default()
                }
            ),
            Err(LogCanisterError::InvalidConfiguration(
                "error parsing logger filter: invalid logging spec 'invalid'".into()
            ))
        );
    }

    #[test]
    fn reload_loads_stored_settings() {
        let mut state = test_state();

        // Simulate canister reload
        LOGGER_CONFIG.with(|v| *v.borrow_mut() = None);
        state.settings = None;

        state.reload(test_memory()).unwrap();

        assert_eq!(state.get_settings(), test_canister_settings());
    }

    #[test]
    fn reload_configures_logger() {
        let mut state = test_state();

        // Simulate canister reload
        LOGGER_CONFIG.with(|v| *v.borrow_mut() = None);
        state.settings = None;

        state.reload(test_memory()).unwrap();

        assert!(LOGGER_CONFIG.with(|v| v.borrow().is_some()));
    }

    #[test]
    fn reload_fails_if_already_initialized() {
        let mut state = test_state();
        assert_eq!(
            state.reload(test_memory()),
            Err(LogCanisterError::AlreadyInitialized)
        );
    }

    #[test]
    fn add_permission_works() {
        let mut state = test_state();
        assert!(state
            .check_permission(user(), LoggerPermission::Read)
            .is_err());
        assert!(state
            .check_permission(user(), LoggerPermission::Configure)
            .is_err());

        state
            .add_permission(admin(), user(), LoggerPermission::Read)
            .unwrap();
        assert!(state
            .check_permission(user(), LoggerPermission::Read)
            .is_ok());
        assert!(state
            .check_permission(user(), LoggerPermission::Configure)
            .is_err());

        state
            .add_permission(admin(), user(), LoggerPermission::Configure)
            .unwrap();
        assert!(state
            .check_permission(user(), LoggerPermission::Read)
            .is_ok());
        assert!(state
            .check_permission(user(), LoggerPermission::Configure)
            .is_ok());
    }

    #[test]
    fn add_permission_checks_caller() {
        let mut state = test_state();
        assert_eq!(
            state.add_permission(reader(), user(), LoggerPermission::Configure),
            Err(LogCanisterError::NotAuthorized)
        );
    }

    #[test]
    fn add_permission_duplicates_is_noop() {
        let mut state = test_state();

        assert!(state
            .check_permission(reader(), LoggerPermission::Read)
            .is_ok());
        state
            .add_permission(admin(), reader(), LoggerPermission::Read)
            .unwrap();

        assert!(state
            .check_permission(reader(), LoggerPermission::Read)
            .is_ok());
        assert!(state
            .check_permission(reader(), LoggerPermission::Configure)
            .is_err());
    }

    #[test]
    fn add_permission_upgrade_permission_level() {
        let mut state = test_state();

        assert!(state
            .check_permission(reader(), LoggerPermission::Read)
            .is_ok());
        state
            .add_permission(admin(), reader(), LoggerPermission::Configure)
            .unwrap();

        assert!(state
            .check_permission(reader(), LoggerPermission::Read)
            .is_ok());
        assert!(state
            .check_permission(reader(), LoggerPermission::Configure)
            .is_ok());
    }

    #[test]
    fn add_permission_downgrade_permission_level_is_noop() {
        let mut state = test_state();

        assert!(state
            .check_permission(admin(), LoggerPermission::Read)
            .is_ok());
        assert!(state
            .check_permission(admin(), LoggerPermission::Configure)
            .is_ok());

        state
            .add_permission(admin(), admin(), LoggerPermission::Read)
            .unwrap();

        assert!(state
            .check_permission(admin(), LoggerPermission::Read)
            .is_ok());
        assert!(state
            .check_permission(admin(), LoggerPermission::Configure)
            .is_ok());
    }

    #[test]
    fn add_permission_saves_value_to_stable_memory() {
        let mut state = test_state();
        state
            .add_permission(admin(), user(), LoggerPermission::Read)
            .unwrap();
        let acl = state.acl();

        reset_config();
        state.reload(test_memory()).unwrap();
        assert_eq!(state.acl(), acl);
    }

    #[test]
    fn configure_permission_grants_read_access() {
        let mut state = test_state();
        state
            .add_permission(admin(), user(), LoggerPermission::Configure)
            .unwrap();
        assert!(state
            .check_permission(user(), LoggerPermission::Configure)
            .is_ok());
    }

    #[test]
    fn remove_permission_works() {
        let mut state = test_state();
        assert!(state
            .check_permission(reader(), LoggerPermission::Read)
            .is_ok());
        assert!(state
            .check_permission(reader(), LoggerPermission::Configure)
            .is_err());

        state
            .remove_permission(admin(), reader(), LoggerPermission::Read)
            .unwrap();

        assert!(state
            .check_permission(reader(), LoggerPermission::Read)
            .is_err());
        assert!(state
            .check_permission(reader(), LoggerPermission::Configure)
            .is_err());
    }

    #[test]
    fn remove_permission_checks_caller() {
        let mut state = test_state();
        assert_eq!(
            state.remove_permission(reader(), admin(), LoggerPermission::Configure),
            Err(LogCanisterError::NotAuthorized)
        );
    }

    #[test]
    fn remove_permission_non_existing_is_nop() {
        let mut state = test_state();
        state
            .remove_permission(admin(), user(), LoggerPermission::Read)
            .unwrap();
        assert!(state
            .check_permission(user(), LoggerPermission::Read)
            .is_err());
    }

    #[test]
    fn remove_permission_saves_value_to_stable_memory() {
        let mut state = test_state();
        state
            .remove_permission(admin(), reader(), LoggerPermission::Read)
            .unwrap();
        let acl = state.acl();

        reset_config();
        state.reload(test_memory()).unwrap();
        assert_eq!(state.acl(), acl);
    }

    #[test]
    fn set_logger_filter_checks_caller() {
        let mut state = test_state();
        assert_eq!(
            state.set_logger_filter(user(), "trace".into()),
            Err(LogCanisterError::NotAuthorized)
        );
    }

    #[test]
    fn set_logger_filter_updates_stored_settings() {
        let mut state = test_state();
        let new_filter = "trace".to_string();
        state
            .set_logger_filter(admin(), new_filter.clone())
            .unwrap();
        assert_eq!(state.get_settings().log_filter.unwrap(), new_filter);
    }

    #[test]
    fn set_logger_filter_returns_error_if_invalid_string() {
        let mut state = test_state();
        assert_eq!(
            state.set_logger_filter(admin(), "crate=invalid".into()),
            Err(LogCanisterError::InvalidConfiguration(
                "error parsing logger filter: invalid logging spec 'invalid'".into()
            ))
        );
    }

    #[test]
    fn set_logger_filter_stores_config_to_stable_memory() {
        let mut state = test_state();
        state
            .set_logger_filter(admin(), "debug,crate1=warn".into())
            .unwrap();
        let settings = state.get_settings();

        reset_config();
        state.settings = None;
        state.reload(test_memory()).unwrap();

        assert_eq!(state.get_settings(), settings);
    }

    #[test]
    fn get_logs_checks_permissions() {
        let state = test_state();
        let _ = state
            .get_logs(
                admin(),
                Pagination {
                    offset: 0,
                    count: 10,
                },
            )
            .unwrap();
        let _ = state
            .get_logs(
                reader(),
                Pagination {
                    offset: 0,
                    count: 10,
                },
            )
            .unwrap();

        assert_eq!(
            state.get_logs(
                user(),
                Pagination {
                    offset: 0,
                    count: 10
                }
            ),
            Err(LogCanisterError::NotAuthorized)
        );
    }

    #[test]
    fn set_in_memory_records_checks_permissions() {
        let mut state = test_state();
        assert!(state.set_in_memory_records(admin(), 10).is_ok());
        assert_eq!(
            state.set_in_memory_records(reader(), 10),
            Err(LogCanisterError::NotAuthorized)
        );
        assert_eq!(
            state.set_in_memory_records(user(), 10),
            Err(LogCanisterError::NotAuthorized)
        );
    }

    #[test]
    fn set_in_memory_records_updates_settings() {
        let mut state = test_state();
        state.set_in_memory_records(admin(), 10).unwrap();
        assert_eq!(state.get_settings().in_memory_records.unwrap(), 10);
    }

    #[test]
    fn set_in_memory_records_changes_logger_capacity() {
        let mut state = test_state();
        state.set_in_memory_records(admin(), 0).unwrap();
        assert!(!InMemoryWriter::is_enabled());
    }

    #[test]
    fn set_in_memory_records_stores_value_in_stable_memory() {
        let mut state = test_state();
        state.set_in_memory_records(admin(), 42).unwrap();
        let settings = state.get_settings();

        reset_config();
        state.settings = None;
        state.reload(test_memory()).unwrap();

        assert_eq!(state.get_settings(), settings);
    }
}
