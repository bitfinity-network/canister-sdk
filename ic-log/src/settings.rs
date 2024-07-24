use candid::{CandidType, Principal};
use serde::Deserialize;

use crate::did::{LogCanisterSettings, LoggerAcl, LoggerPermission};

const DEFAULT_IN_MEMORY_RECORDS: usize = 1024;
const DEFAULT_MAX_RECORD_LENGTH: usize = 1024;

/// Logger settings.
///
/// For details about the fields, see docs of [`LogCanisterSettings`].
#[derive(Debug, Clone, PartialEq, Eq, CandidType, Deserialize)]
pub struct LogSettingsV2 {
    pub enable_console: bool,
    pub in_memory_records: usize,
    pub max_record_length: usize,
    pub log_filter: String,
    pub acl: LoggerAcl,
}

impl Default for LogSettingsV2 {
    fn default() -> Self {
        Self {
            enable_console: true,
            in_memory_records: DEFAULT_IN_MEMORY_RECORDS,
            max_record_length: DEFAULT_MAX_RECORD_LENGTH,
            log_filter: "debug".to_string(),
            acl: Default::default(),
        }
    }
}

impl LogSettingsV2 {
    pub fn from_did(settings: LogCanisterSettings, owner: Principal) -> Self {
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
            acl: settings
                .acl
                .unwrap_or_else(|| [(owner, LoggerPermission::Configure)].into()),
        }
    }
}

impl From<LogSettingsV2> for LogCanisterSettings {
    fn from(value: LogSettingsV2) -> Self {
        Self {
            enable_console: Some(value.enable_console),
            in_memory_records: Some(value.in_memory_records),
            max_record_length: Some(value.max_record_length),
            log_filter: Some(value.log_filter),
            acl: Some(value.acl),
        }
    }
}
