use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use candid::Principal;
use ic_canister::{generate_idl, init, query, update, Canister, Idl, PreUpdate};
use ic_exports::ic_kit::ic;
use ic_log::did::LoggerPermission;
use ic_log::writer::Logs;
use ic_log::{init_log, LogSettings, LoggerConfig};
use log::{debug, error, info};

#[derive(Canister)]
pub struct LogCanister {
    #[id]
    id: Principal,
}

impl PreUpdate for LogCanister {}

impl LogCanister {
    #[init]
    pub fn init(&self) {
        let settings = LogSettings {
            in_memory_records: 128,
            max_record_length: 1024,
            log_filter: "info".to_string(),
            enable_console: true,
            acl: [
                (ic::caller(), LoggerPermission::Read),
                (ic::caller(), LoggerPermission::Configure),
            ]
            .into(),
        };
        match init_log(&settings) {
            Ok(logger_config) => LoggerConfigService::default().init(logger_config),
            Err(err) => {
                ic_exports::ic_cdk::println!("error configuring the logger. Err: {:?}", err)
            }
        }
        info!("LogCanister initialized");
    }

    #[query]
    pub fn get_log_records(&self, count: usize) -> Logs {
        debug!("collecting {count} log records");
        ic_log::take_memory_records(count, 0)
    }

    #[update]
    pub async fn log_info(&self, text: String) {
        info!("{text}");
    }

    #[update]
    pub async fn log_debug(&self, text: String) {
        debug!("{text}");
    }

    #[update]
    pub async fn log_error(&self, text: String) {
        error!("{text}");
    }

    #[update]
    pub async fn set_logger_filter(&self, filter: String) {
        LoggerConfigService::default().set_logger_filter(&filter);
        debug!("log filter set to {filter}");
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}

type ForceNotSendAndNotSync = PhantomData<Rc<()>>;

thread_local! {
    static LOGGER_CONFIG: RefCell<Option<LoggerConfig>> = const { RefCell::new(None) };
}

#[derive(Debug, Default)]
/// Handles the runtime logger configuration
pub struct LoggerConfigService(ForceNotSendAndNotSync);

impl LoggerConfigService {
    /// Sets a new LoggerConfig
    pub fn init(&self, logger_config: LoggerConfig) {
        LOGGER_CONFIG.with(|config| config.borrow_mut().replace(logger_config));
    }

    /// Changes the logger filter at runtime
    pub fn set_logger_filter(&self, filter: &str) {
        LOGGER_CONFIG.with(|config| match *config.borrow_mut() {
            Some(ref logger_config) => {
                logger_config.update_filters(filter);
            }
            None => panic!("LoggerConfig not initialized"),
        });
    }
}

fn main() {
    let canister_e_idl = LogCanister::idl();
    let idl = candid::pretty::candid::compile(&canister_e_idl.env.env, &Some(canister_e_idl.actor));

    println!("{}", idl);
}
