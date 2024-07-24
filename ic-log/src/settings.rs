use candid::CandidType;
use serde::Deserialize;

use crate::did::{LogCanisterSettings, LoggerAcl};

const DEFAULT_IN_MEMORY_RECORDS: usize = 1024;
const DEFAULT_MAX_RECORD_LENGTH: usize = 1024;

/// Log settings to initialize the logger
///
/// This structure is used to configure canisters that use `ic-log` of version `0.18` or below.
/// For newer versions of the library, use [`LogSettingsV2`] for logger configuration and
/// [`LogCanisterSettings`] for canister initialization.
#[derive(Default, Debug, Clone, CandidType, Deserialize)]
pub struct LogSettings {
    /// Enable logging to console (`ic::print` when running in IC)
    pub enable_console: bool,
    /// Number of records to be stored in the circular memory buffer.
    /// If None - storing records will be disable.
    /// If Some - should be power of two.
    pub in_memory_records: Option<usize>,
    /// Log configuration as combination of filters. By default the logger is OFF.
    /// Example of valid configurations:
    /// - info
    /// - debug,crate1::mod1=error,crate1::mod2,crate2=debug
    pub log_filter: Option<String>,
}

/// Logger settings.
///
/// For details about the fields, see docs of [`LogCanisterSettings`].
#[derive(Debug, Clone, PartialEq, Eq, CandidType, Deserialize)]
pub struct LogSettingsV2 {
    pub enable_console: bool,
    pub in_memory_records: usize,
    pub max_record_length: usize,
    pub log_filter: String,
}

impl Default for LogSettingsV2 {
    fn default() -> Self {
        Self {
            enable_console: true,
            in_memory_records: DEFAULT_IN_MEMORY_RECORDS,
            max_record_length: DEFAULT_MAX_RECORD_LENGTH,
            log_filter: "debug".to_string(),
        }
    }
}

impl From<LogCanisterSettings> for LogSettingsV2 {
    fn from(settings: LogCanisterSettings) -> Self {
        let default = Self::default();
        Self {
            enable_console: settings.enable_console.unwrap_or(default.enable_console),
            in_memory_records: settings
                .in_memory_records
                .unwrap_or(default.in_memory_records),
            max_record_length: settings
                .max_record_length
                .unwrap_or(default.max_record_length),
            log_filter: settings.log_filter.unwrap_or(default.log_filter),
        }
    }
}

impl From<(LogSettingsV2, LoggerAcl)> for LogCanisterSettings {
    fn from((value, acl): (LogSettingsV2, LoggerAcl)) -> Self {
        Self {
            enable_console: Some(value.enable_console),
            in_memory_records: Some(value.in_memory_records),
            max_record_length: Some(value.max_record_length),
            log_filter: Some(value.log_filter),
            acl: Some(acl),
        }
    }
}
