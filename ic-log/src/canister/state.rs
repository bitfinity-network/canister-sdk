use std::borrow::Cow;
use std::cell::RefCell;

use candid::{Decode, Encode, Principal};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{Bound, CellStructure, IcMemoryManager, MemoryId, StableCell, Storable};
use ic_storage::IcStorage;

use crate::did::{LogCanisterError, LogCanisterSettings, LoggerAcl, LoggerPermission, Pagination};
use crate::writer::{InMemoryWriter, Logs};
use crate::{take_memory_records, LogSettings, LoggerConfig};

thread_local! {
    static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());
    static LOGGER_CONFIG: RefCell<Option<LoggerConfig>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, IcStorage)]
pub struct LogState {
    settings: LogSettings,
    memory_id: MemoryId,
}

impl Default for LogState {
    fn default() -> Self {
        Self {
            settings: LogSettings::default(),
            memory_id: Self::INVALID_MEMORY_ID,
        }
    }
}

impl LogState {
    const INVALID_MEMORY_ID: MemoryId = MemoryId::new(254);

    pub fn new(memory_id: MemoryId, acl: LoggerAcl) -> Self {
        let mut this = Self::default();
        this.memory_id = memory_id;
        this.settings.acl = acl;

        this
    }

    pub fn init(
        &mut self,
        caller: Principal,
        memory_id: MemoryId,
        log_settings: LogCanisterSettings,
    ) -> Result<(), LogCanisterError> {
        if LOGGER_CONFIG.with(|logger_config| logger_config.borrow().is_some()) {
            return Err(LogCanisterError::AlreadyInitialized);
        }

        self.settings = LogSettings::from_did(log_settings, caller);
        self.memory_id = memory_id;

        self.store()?;

        Self::init_log(&self.settings)?;

        // Print this out without using log in case the given parameters prevent logs to be printed.
        #[cfg(target_arch = "wasm32")]
        ic_exports::ic_kit::ic::print(format!(
            "Initialized logging with settings: {log_settings:?}"
        ));

        Ok(())
    }

    pub fn get_settings(&self) -> &LogSettings {
        &self.settings
    }

    pub fn set_logger_filter(
        &mut self,
        caller: Principal,
        filter_value: String,
    ) -> Result<(), LogCanisterError> {
        self.check_permission(caller, LoggerPermission::Configure)?;

        self.settings.log_filter = filter_value.clone();
        LOGGER_CONFIG.with(|config| {
            if let Some(config) = &mut *config.borrow_mut() {
                config.update_filters(&filter_value);
            }
        });

        self.store().expect("Failed to update logger filter");

        log::info!("Updated log filter to: {filter_value:?}");

        Ok(())
    }

    pub fn set_in_memory_records(
        &mut self,
        caller: Principal,
        count: usize,
    ) -> Result<(), LogCanisterError> {
        self.check_permission(caller, LoggerPermission::Configure)?;

        self.settings.in_memory_records = count;
        InMemoryWriter::change_capacity(count);

        Ok(())
    }

    pub fn get_logs(&self, caller: Principal, page: Pagination) -> Result<Logs, LogCanisterError> {
        self.check_permission(caller, LoggerPermission::Read)?;
        Ok(take_memory_records(page.count, page.offset))
    }

    pub fn reload(&mut self, memory_id: MemoryId) -> Result<(), LogCanisterError> {
        if LOGGER_CONFIG.with(|logger_config| logger_config.borrow().is_some()) {
            return Err(LogCanisterError::AlreadyInitialized);
        }

        if memory_id == Self::INVALID_MEMORY_ID {
            return Err(LogCanisterError::InvalidMemoryId);
        }

        let settings = MEMORY_MANAGER.with(|mm| {
            Ok(StableCell::new(
                mm.get(memory_id),
                StorableLogSettings(LogSettings::default()),
            )
            .map_err(|err| {
                LogCanisterError::Generic(format!(
                    "Failed to write log config to the stable storage: {err:?}"
                ))
            })?
            .get()
            .clone())
        })?;

        if settings.0 == LogSettings::default() {
            return Err(LogCanisterError::InvalidMemoryId);
        }

        self.settings = settings.0;
        self.memory_id = memory_id;

        Self::init_log(&self.settings)?;

        Ok(())
    }

    pub fn add_permission(
        &mut self,
        caller: Principal,
        to: Principal,
        permission: LoggerPermission,
    ) -> Result<(), LogCanisterError> {
        self.check_permission(caller, LoggerPermission::Configure)?;
        self.settings.acl.insert((to, permission));
        Ok(())
    }

    pub fn remove_permission(
        &mut self,
        caller: Principal,
        from: Principal,
        permission: LoggerPermission,
    ) -> Result<(), LogCanisterError> {
        self.check_permission(caller, LoggerPermission::Configure)?;
        self.settings.acl.remove(&(from, permission));
        Ok(())
    }

    fn store(&self) -> Result<(), LogCanisterError> {
        let memory_id = self.memory_id;
        if memory_id == Self::INVALID_MEMORY_ID {
            return Err(LogCanisterError::InvalidMemoryId);
        }

        let log_settings = self.settings.clone();
        MEMORY_MANAGER
            .with(|mm| {
                let mut cell = StableCell::new(
                    mm.get(memory_id),
                    StorableLogSettings(LogSettings::default()),
                )?;

                cell.set(StorableLogSettings(log_settings))
            })
            .map_err(|err| {
                LogCanisterError::Generic(format!(
                    "Failed to write log config to the stable storage: {err:?}"
                ))
            })
    }

    fn init_log(_log_settings: &LogSettings) -> Result<(), LogCanisterError> {
        let logger_config = {
            cfg_if::cfg_if! {
                if #[cfg(test)] {
                    let (_, config) = crate::Builder::default().build();
                    config
                } else {
                    crate::init_log(_log_settings).map_err(|_| LogCanisterError::AlreadyInitialized)?
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
                self.settings
                    .acl
                    .contains(&(caller, LoggerPermission::Read))
                    || (self
                        .settings
                        .acl
                        .contains(&(caller, LoggerPermission::Configure)))
            }
            LoggerPermission::Configure => self
                .settings
                .acl
                .contains(&(caller, LoggerPermission::Configure)),
        };

        if allowed {
            Ok(())
        } else {
            Err(LogCanisterError::NotAuthorized)
        }
    }
}

#[derive(Debug, Clone)]
pub struct StorableLogSettings(pub LogSettings);

impl Storable for StorableLogSettings {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::from(Encode!(&self.0).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Self(Decode!(&bytes, LogSettings).unwrap())
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

    fn test_settings() -> LogSettings {
        LogSettings {
            enable_console: true,
            in_memory_records: 10,
            max_record_length: 1024,
            log_filter: "trace".to_string(),
            acl: [
                (admin(), LoggerPermission::Configure),
                (reader(), LoggerPermission::Read),
            ]
            .into(),
        }
    }

    fn test_state() -> LogState {
        let mut state = LogState::default();
        state
            .init(admin(), test_memory(), test_settings().into())
            .unwrap();
        state
    }

    #[test]
    fn init_stores_settings() {
        let mut state = LogState::default();
        let settings = LogSettings {
            enable_console: true,
            in_memory_records: 10,
            max_record_length: 1024,
            log_filter: "debug".to_string(),
            acl: [(admin(), LoggerPermission::Configure)].into(),
        };
        state
            .init(admin(), MemoryId::new(1), settings.clone().into())
            .unwrap();

        assert_eq!(state.get_settings(), &settings);
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
            state.init(admin(), test_memory(), test_settings().into()),
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
                LogSettings::default().into()
            ),
            Err(LogCanisterError::InvalidMemoryId)
        );
    }

    #[test]
    fn reload_loads_stored_settings() {
        let mut state = test_state();

        // Simulate canister reload
        LOGGER_CONFIG.with(|v| *v.borrow_mut() = None);
        state.settings = LogSettings::default();

        state.reload(test_memory()).unwrap();

        assert_eq!(state.settings, test_settings());
    }

    #[test]
    fn reload_configures_logger() {
        let mut state = test_state();

        // Simulate canister reload
        LOGGER_CONFIG.with(|v| *v.borrow_mut() = None);
        state.settings = LogSettings::default();

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
        state.settings = LogSettings::default();

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
    fn update_logger_filter_checks_caller() {
        let mut state = test_state();
        assert_eq!(
            state.set_logger_filter(user(), "trace".into()),
            Err(LogCanisterError::NotAuthorized)
        );
    }

    #[test]
    fn update_logger_filter_updates_stored_settings() {
        let mut state = test_state();
        let new_filter = "trace".to_string();
        state
            .set_logger_filter(admin(), new_filter.clone())
            .unwrap();
        assert_eq!(state.get_settings().log_filter, new_filter);
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
        assert_eq!(state.get_settings().in_memory_records, 10);
    }

    #[test]
    fn set_in_memory_records_changes_logger_capacity() {
        let mut state = test_state();
        state.set_in_memory_records(admin(), 0).unwrap();
        assert!(!InMemoryWriter::is_enabled());
    }
}
