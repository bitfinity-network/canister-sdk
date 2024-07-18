use std::collections::HashSet;

use candid::CandidType;
use ic_exports::ic_kit::Principal;
use serde::Deserialize;

#[derive(Debug, Copy, Clone, CandidType, Deserialize)]
pub struct Pagination {
    pub offset: usize,
    pub count: usize,
}

#[derive(Debug, Clone, CandidType, Deserialize, Eq, PartialEq)]
pub enum LogCanisterError {
    AlreadyInitialized,
    NotAuthorized,
    Generic(String),
    InvalidMemoryId,
}

#[derive(Debug, Clone, Copy, CandidType, Deserialize, Eq, PartialEq, Hash)]
pub enum LoggerPermission {
    Read,
    Configure,
}

pub type LoggerAcl = HashSet<(Principal, LoggerPermission)>;

/// Log settings to initialize the logger
#[derive(Default, Debug, Clone, CandidType, Deserialize, PartialEq, Eq)]
pub struct LogSettings {
    /// Enable logging to console (`ic::print` when running in IC)
    pub enable_console: bool,
    /// Number of records to be stored in the circular memory buffer.
    /// If None - storing records will be disable.
    /// If Some - should be power of two.
    pub in_memory_records: usize,
    /// Log configuration as combination of filters. By default the logger is OFF.
    /// Example of valid configurations:
    /// - info
    /// - debug,crate1::mod1=error,crate1::mod2,crate2=debug
    pub log_filter: String,

    pub acl: LoggerAcl,
}
