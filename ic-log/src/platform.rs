use std::time::SystemTime;

/// Returns the current SystemTime
#[inline]
pub fn current_system_time() -> SystemTime {
    #[cfg(not(target_family = "wasm"))]
    {
        SystemTime::now()
    }

    #[cfg(target_family = "wasm")]
    {
        let timestamp_in_nanos = ic_exports::ic_cdk::api::time();
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_nanos(timestamp_in_nanos)
    }
}

/// Prints to the standard out
#[inline]
pub fn print(data: &[u8]) {
    #[cfg(not(target_family = "wasm"))]
    {
        print!("{}", String::from_utf8_lossy(data))
    }

    #[cfg(target_family = "wasm")]
    {
        ic_exports::ic_cdk::print(String::from_utf8_lossy(data))
    }
}
