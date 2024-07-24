use std::collections::HashSet;

use candid::CandidType;
use env_filter::ParseError;
use ic_exports::ic_kit::Principal;
use log::SetLoggerError;
use serde::Deserialize;

/// Specifies what to take from a long list of items.
#[derive(Debug, Copy, Clone, CandidType, Deserialize)]
pub struct Pagination {
    /// First item id to get.
    pub offset: usize,
    /// Max number of items to get.
    pub count: usize,
}

/// Error returned by the logger canister.
#[derive(Debug, Clone, CandidType, Deserialize, Eq, PartialEq)]
pub enum LogCanisterError {
    /// An initialization was called for the logger, but it is already initialized.
    AlreadyInitialized,
    /// The logger is not initialized.
    NotInitialized,
    /// The caller does not have permission to execute this method.
    NotAuthorized,
    /// Something bad happened.
    Generic(String),
    /// The given memory ID cannot be used to store logger configuration.
    InvalidMemoryId,
    /// Error in the logger configuration.
    InvalidConfiguration(String),
}

impl From<ParseError> for LogCanisterError {
    fn from(value: ParseError) -> Self {
        Self::InvalidConfiguration(value.to_string())
    }
}

impl From<SetLoggerError> for LogCanisterError {
    fn from(_: SetLoggerError) -> Self {
        Self::AlreadyInitialized
    }
}

/// Permission of a caller for logger canister operations.
#[derive(Debug, Clone, Copy, CandidType, Deserialize, Eq, PartialEq, Hash)]
pub enum LoggerPermission {
    /// Allows the caller to get the logs.
    Read,
    /// Allows the caller to get the logs and change the configuration of the canister.
    Configure,
}

pub type LoggerAcl = HashSet<(Principal, LoggerPermission)>;

/// Log settings to initialize the logger
#[derive(Default, Debug, Clone, CandidType, Deserialize, PartialEq, Eq)]
pub struct LogCanisterSettings {
    /// Enable logging to console (`ic::print` when running in IC)
    pub enable_console: Option<bool>,

    /// Number of records to be stored in the circular memory buffer.
    ///
    /// If set to 0, logging will be disabled.
    ///
    /// If `None`, default value will be used (`1024`).
    pub in_memory_records: Option<usize>,

    /// Maximum length (in bytes) of a single log entry.
    ///
    /// If set to 0, the log will still add entries to the log, but they all will contain only an
    /// empty string.
    ///
    /// If `None`, default value will be used (`1024`).
    pub max_record_length: Option<usize>,

    /// Log configuration as combination of filters. By default, the logger filter is set to `warn`.
    ///
    /// Example of valid configurations:
    /// - info
    /// - debug,crate1::mod1=error,crate1::mod2,crate2=debug
    pub log_filter: Option<String>,

    /// Access control list for the logs.
    ///
    /// Of set to `None`, the creator of the canister will be assigned `Configure` permission.
    pub acl: Option<LoggerAcl>,
}
