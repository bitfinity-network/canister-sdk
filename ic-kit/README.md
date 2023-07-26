# IC Kit

[![Docs](https://docs.rs/ic-kit/badge.svg)](https://docs.rs/ic-kit)

This library provides an alternative to `ic-cdk` that can help developers write canisters
and unit test them in their Rust code.

## Install

Add this to your `Cargo.toml`

```toml
[dependencies]
ic-kit = "0.4.3"
ic-cdk = "0.3.1"

[target.'cfg(not(target_family = "wasm"))'.dependencies]
async-std = { version="1.10.0", features = ["attributes"] }
```

## Example Usage

```rust
use ic_kit::macros::*;
use ic_kit::{ic, Principal};

#[update]
fn whoami() -> Principal {
    ic::caller()
}

#[update]
async fn send_cycles(canister_id: Principal, cycles: u64) -> Result<(), String> {
    ic::call_with_payment(canister_id, "wallet_accept", (), cycles)
        .await
        .map_err(|(code, msg)| format!("Call failed with code={}: {}", code as u8, msg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ic_kit::{mock_principals, MockContext};

    #[test]
    fn test_whoami() {
        MockContext::new()
            .with_caller(mock_principals::alice())
            .inject();

        assert_eq!(whoami(), mock_principals::alice());
    }

    #[async_test]
    async fn test_send_cycles() {
        // Create a context that just consumes 1000 cycles from all the inter-canister calls and
        // returns "()" in response.
        let ctx = MockContext::new()
            .with_consume_cycles_handler(1000)
            .inject();

        // Init a watcher at this point that will track all of the calls made from now on.
        let watcher = ctx.watch();

        send_cycles(mock_principals::xtc(), 5000).await.unwrap();

        assert_eq!(watcher.cycles_consumed(), 1000);
        assert_eq!(watcher.cycles_sent(), 5000);
    }
}
```