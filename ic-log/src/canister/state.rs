use std::borrow::Cow;
use std::cell::RefCell;

use candid::{Decode, Encode, Principal};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{Bound, CellStructure, IcMemoryManager, MemoryId, StableCell, Storable};
use ic_storage::IcStorage;

use crate::did::{LogCanisterError, LogCanisterSettings, LoggerAcl, LoggerPermission, Pagination};
use crate::writer::{InMemoryWriter, Logs};
use crate::{take_memory_records, LogSettingsV2, LoggerConfig};

thread_local! {
    static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());
    static LOGGER_CONFIG: RefCell<Option<LoggerConfig>> = const { RefCell::new(None) };
}

/// State of the logger canister.
///
/// Before logger can be used, it must be initialized with the [`LogState::init`] method.
#[derive(Debug, Clone, IcStorage)]
pub struct LogState {
    settings: LogSettingsV2,
    acl: LoggerAcl,
    memory_id: MemoryId,
}

impl Default for LogState {
    fn default() -> Self {
        Self {
            settings: LogSettingsV2::default(),
            acl: LoggerAcl::default(),
            memory_id: Self::INVALID_MEMORY_ID,
        }
    }
}

impl LogState {
    const INVALID_MEMORY_ID: MemoryId = MemoryId::new(254);

    /// Creates a new instance of the state. This method is usually not needed for implementing a
    /// `LogCanister` trait, as the state can be taken by the [`IcStorage::get()`] method instead.
    pub fn new(memory_id: MemoryId, acl: LoggerAcl) -> Self {
        Self {
            acl,
            memory_id,
            ..Default::default()
        }
    }

    /// Initializes the logger with the given settings.
    ///
    /// IMPORTANT: this method must be called only one during the runtime of the application. A
    /// canister process can be started with `#[init]` and `#[post_upgrade]` methods. In case of
    /// `post_upgrade`, use [`LogState::reload`] method instead.
    ///
    /// # Arguments
    /// * `caller` - caller of the `#[init]` method of the canister. This principal is used to
    ///   create a default ACL in case one is not given in the configuration.
    /// * `memory_id` - stable memory id to use for the logger configuration.
    /// * `log_settings` - logger settings.
    ///
    /// # Errors
    ///
    /// Returns [`LogCanisterError::AlreadyInitialized`] if called more than once during the
    /// lifetime of the application.
    pub fn init(
        &mut self,
        caller: Principal,
        memory_id: MemoryId,
        log_settings: LogCanisterSettings,
    ) -> Result<(), LogCanisterError> {
        if LOGGER_CONFIG.with(|logger_config| logger_config.borrow().is_some()) {
            return Err(LogCanisterError::AlreadyInitialized);
        }

        self.acl = log_settings
            .acl
            .clone()
            .unwrap_or_else(|| [(caller, LoggerPermission::Configure)].into());
        self.settings = log_settings.into();
        self.memory_id = memory_id;

        Self::init_log(&self.settings)?;

        self.store()?;

        // Print this out without using log in case the given parameters prevent logs to be printed.
        #[cfg(target_arch = "wasm32")]
        ic_exports::ic_kit::ic::print(format!(
            "Initialized logging with settings: {:?}",
            self.settings
        ));

        Ok(())
    }

    /// Returns current settings of the logger.
    pub fn get_settings(&self) -> LogCanisterSettings {
        (self.settings.clone(), self.acl.clone()).into()
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

        self.settings.log_filter.clone_from(&filter_value);

        self.store().expect("failed to update logger filter");

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

        self.settings.in_memory_records = count;
        InMemoryWriter::change_capacity(count);

        self.store().expect("failed to update in memory records");

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
    pub fn reload(&mut self, memory_id: MemoryId) -> Result<(), LogCanisterError> {
        if LOGGER_CONFIG.with(|logger_config| logger_config.borrow().is_some()) {
            return Err(LogCanisterError::AlreadyInitialized);
        }

        if memory_id == Self::INVALID_MEMORY_ID {
            return Err(LogCanisterError::InvalidMemoryId);
        }

        let settings = MEMORY_MANAGER.with(|mm| {
            StableCell::new(
                mm.get(memory_id),
                StorableLogSettings(LogSettingsV2::default(), LoggerAcl::default()),
            )
            .map_err(|err| {
                LogCanisterError::Generic(format!(
                    "Failed to write log config to the stable storage: {err:?}"
                ))
            })
            .map(|v| v.get().clone())
        })?;

        if settings.0 == LogSettingsV2::default() {
            return Err(LogCanisterError::InvalidMemoryId);
        }

        self.settings = settings.0;
        self.acl = settings.1;
        self.memory_id = memory_id;

        Self::init_log(&self.settings)?;

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
        self.acl.insert((to, permission));

        self.store().expect("failed to update stable storage");
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
        self.acl.remove(&(from, permission));

        self.store().expect("failed to update stable storage");
        Ok(())
    }

    fn store(&self) -> Result<(), LogCanisterError> {
        let memory_id = self.memory_id;
        if memory_id == Self::INVALID_MEMORY_ID {
            return Err(LogCanisterError::InvalidMemoryId);
        }

        let log_settings = self.settings.clone();
        let acl = self.acl.clone();
        MEMORY_MANAGER
            .with(|mm| {
                let mut cell = StableCell::new(
                    mm.get(memory_id),
                    StorableLogSettings(LogSettingsV2::default(), LoggerAcl::default()),
                )?;

                cell.set(StorableLogSettings(log_settings, acl))
            })
            .map_err(|err| {
                LogCanisterError::Generic(format!(
                    "Failed to write log config to the stable storage: {err:?}"
                ))
            })
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
        let allowed = match logger_permission {
            LoggerPermission::Read => {
                self.acl.contains(&(caller, LoggerPermission::Read))
                    || (self.acl.contains(&(caller, LoggerPermission::Configure)))
            }
            LoggerPermission::Configure => {
                self.acl.contains(&(caller, LoggerPermission::Configure))
            }
        };

        if allowed {
            Ok(())
        } else {
            Err(LogCanisterError::NotAuthorized)
        }
    }
}

#[derive(Debug, Clone)]
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
    use super::*;

    fn admin() -> Principal {
        Principal::from_slice(&[1; 20])
    }

    fn reader() -> Principal {
        Principal::from_slice(&[2; 20])
    }

    fn user() -> Principal {
        Principal::from_slice(&[5; 20])
    }

    fn test_memory() -> MemoryId {
        MemoryId::new(2)
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
                MemoryId::new(1),
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
    fn init_fails_if_default_memory_id() {
        let mut state = LogState::default();
        assert_eq!(
            state.init(
                admin(),
                LogState::INVALID_MEMORY_ID,
                test_canister_settings(),
            ),
            Err(LogCanisterError::InvalidMemoryId)
        );
    }

    #[test]
    fn init_fails_with_invalid_filter_string() {
        let mut state = LogState::default();
        assert_eq!(
            state.init(
                admin(),
                MemoryId::new(1),
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
        state.settings = LogSettingsV2::default();
        state.acl = LoggerAcl::default();

        state.reload(test_memory()).unwrap();

        assert_eq!(state.settings, test_settings());
        assert_eq!(state.acl, test_acl());
    }

    #[test]
    fn reload_configures_logger() {
        let mut state = test_state();

        // Simulate canister reload
        LOGGER_CONFIG.with(|v| *v.borrow_mut() = None);
        state.settings = LogSettingsV2::default();

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
    fn reload_fails_if_incorrect_memory_id() {
        let mut state = test_state();

        // Simulate canister reload
        LOGGER_CONFIG.with(|v| *v.borrow_mut() = None);
        state.settings = LogSettingsV2::default();

        assert_eq!(
            state.reload(MemoryId::new(42)),
            Err(LogCanisterError::InvalidMemoryId)
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
        let acl = state.acl.clone();

        reset_config();
        state.reload(test_memory()).unwrap();
        assert_eq!(state.acl, acl);
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
        let acl = state.acl.clone();

        reset_config();
        state.reload(test_memory()).unwrap();
        assert_eq!(state.acl, acl);
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
        let settings = state.settings.clone();

        reset_config();
        state.reload(test_memory()).unwrap();
        assert_eq!(state.settings, settings);
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
        let settings = state.settings.clone();

        reset_config();
        state.reload(test_memory()).unwrap();
        assert_eq!(state.settings, settings);
    }
}
