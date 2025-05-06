use ic_kit::macros::*;
use ic_kit::{ic, Principal};

#[update]
fn whoami() -> Principal {
    ic::caller()
}

#[update]
async fn send_cycles(canister_id: Principal, cycles: u128) -> Result<(), String> {
    ic::call_with_payment(canister_id, "wallet_accept", (), cycles)
        .await
        .map_err(|err| format!("Call failed {err:?}"))
}

#[cfg(test)]
mod tests {
    use ic_kit::{mock_principals, MockContext};

    use super::*;

    #[test]
    fn test_whoami() {
        MockContext::new()
            .with_caller(mock_principals::alice())
            .inject();

        assert_eq!(whoami(), mock_principals::alice());
    }

    #[tokio::test]
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

    #[tokio::test]
    async fn test_spawn() {
        let ctx = MockContext::new()
            .with_consume_cycles_handler(1000)
            .inject();

        let watcher = ctx.watch();

        ic::spawn(async_job(mock_principals::xtc(), 5000));

        assert_eq!(watcher.cycles_consumed(), 1000);
        assert_eq!(watcher.cycles_sent(), 5000);
    }
}

fn main() {}
