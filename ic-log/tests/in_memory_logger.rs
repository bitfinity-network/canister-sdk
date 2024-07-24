use std::sync::Once;

use candid::Principal;
use ic_log::canister::LogState;
use ic_log::did::{LogCanisterSettings, LoggerAcl, LoggerPermission, Pagination};
use ic_log::LogSettingsV2;
use ic_stable_structures::MemoryId;

fn admin() -> Principal {
    Principal::from_slice(&[1; 20])
}

fn reader() -> Principal {
    Principal::from_slice(&[2; 20])
}

fn test_memory() -> MemoryId {
    MemoryId::new(2)
}

fn test_settings() -> LogSettingsV2 {
    LogSettingsV2 {
        enable_console: true,
        in_memory_records: 10,
        max_record_length: 1024,
        log_filter: "info,ic_log=off".to_string(),
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
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mut state = LogState::default();
        state
            .init(admin(), test_memory(), test_canister_settings())
            .unwrap()
    });

    let mut state = LogState::new(test_memory(), test_acl());
    state
        .set_logger_filter(admin(), test_settings().log_filter)
        .unwrap();
    state
}

#[test]
fn set_logger_filter_updates_filter() {
    let mut state = test_state();
    log::warn!("warn");
    log::error!("error");

    let logs = state
        .get_logs(
            admin(),
            Pagination {
                offset: 0,
                count: 20,
            },
        )
        .unwrap();
    assert_eq!(logs.all_logs_count, 2);

    state
        .set_logger_filter(admin(), "info,ic_log=off,in_memory_logger=error".into())
        .unwrap();

    log::debug!("warn");
    log::error!("error2");

    let logs = state
        .get_logs(
            admin(),
            Pagination {
                offset: 0,
                count: 20,
            },
        )
        .unwrap();

    assert_eq!(logs.all_logs_count, 3);
    assert_eq!(logs.logs.len(), 3);

    assert!(logs.logs[2].log.contains(&"error2".to_string()))
}
