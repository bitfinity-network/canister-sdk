//! This crate exists to work around a problem with `getrandom` 0.2, which is a dependency
//! of `rand` 0.8
//!
//! For the `wasm32-unknown-unknown` target, `getrandom` 0.2 will refuse to compile. This is an
//! intentional policy decision on the part of the getrandom developers. As a consequence, it
//! would not be possible to compile anything which depends on `rand` 0.8 to wasm for use in
//! canister code.
//!
//! Depending on this crate converts the compile time error into a runtime error, by
//! registering a custom `getrandom` implementation. This matches the
//! behavior of `getrandom` 0.1. For code that is not being compiled to
//! `wasm32-unknown-unknown`, this crate has no effect whatsoever.
//!
//! The reason for placing this function into its own dedicated crate is that it not possible
//! to register more than one getrandom implementation. If more than one custom getrandom
//! implementation existed within the source tree, then a canister which depended on two
//! different crates which included the workaround would fail to build due to the conflict.
//!
//! See the [getrandom
//! documentation](https://docs.rs/getrandom/latest/getrandom/macro.register_custom_getrandom.html)
//! for more details on custom implementations.
//!
//! To register this custom implementation, call the function inside of your canister's `init` method
//! or wherever canister initialization happens, with conditional compilation, like this:
//!
//! ```ignore
//! #[init]
//! pub fn init(&mut self) {
//!    #[cfg(target_family = "wasm")]
//!    ic_crypto_getrandom_for_wasm::register_custom_getrandom();
//! }
//! ```

#[cfg(all(
    target_family = "wasm",
    target_vendor = "unknown",
    target_os = "unknown"
))]
pub use custom_getrandom_impl::register_custom_getrandom;

#[cfg(all(
    target_family = "wasm",
    target_vendor = "unknown",
    target_os = "unknown"
))]
mod custom_getrandom_impl {
    use std::cell::RefCell;
    use std::time::Duration;

    use candid::Principal;
    use rand::rngs::StdRng;
    use rand::{RngCore, SeedableRng};

    thread_local! {
        static RNG: RefCell<Option<StdRng>> = const { RefCell::new(None) };
    }

    pub fn register_custom_getrandom() {
        ic_cdk_timers::set_timer(Duration::from_secs(0), || {
            ic_cdk::spawn(generate_randomness())
        });
        getrandom::register_custom_getrandom!(custom_rand);
    }

    fn custom_rand(buf: &mut [u8]) -> Result<(), getrandom::Error> {
        RNG.with(|rng| rng.borrow_mut().as_mut().unwrap().fill_bytes(buf));
        Ok(())
    }

    async fn generate_randomness() {
        let (seed,) = ic_cdk::call(Principal::management_canister(), "raw_rand", ())
            .await
            .unwrap();
        RNG.with(|rng| {
            *rng.borrow_mut() = Some(StdRng::from_seed(seed));
        });
    }
}
