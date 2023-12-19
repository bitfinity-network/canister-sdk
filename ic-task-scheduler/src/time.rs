/// returns the timestamp in seconds
#[inline]
pub fn time_secs() -> u64 {
    #[cfg(not(target_family = "wasm"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("get current timestamp error")
            .as_secs()
    }

    // ic::time() return the nano_sec, we need to change it to sec.
    #[cfg(target_family = "wasm")]
    {
        const E_9: u64 = 10u64.pow(9u32);
        ic_kit::ic::time() / E_9
    }
}
