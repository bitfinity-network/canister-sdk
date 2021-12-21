#[macro_export]
macro_rules! init_api {
    ( $state:ident ) => {
        use ic_helpers::is20::IS20PrincipalExt;

        /// Returns the checksum of a wasm module in hex representation.
        #[query(name = "get_checksum")]
        async fn get_checksum() -> String {
            $state::get().factory.checksum.to_string()
        }

        /// Returns the cycles balances.
        /// If principal == None then cycles balances of factory is returned,
        /// otherwise, cycles balances of `principal` is returned.
        /// If `principal` does not exists, `None` is returned.
        #[update(name = "get_cycles")]
        async fn get_cycles(principal: Option<candid::Principal>) -> Option<candid::Nat> {
            Some(if let Some(principal) = principal {
                ic_helpers::management::Canister::from(principal)
                    .status()
                    .await
                    .map(|status| status.cycles)
                    .ok()?
            } else {
                candid::Principal::cycles()
            })
        }

        /// Accepts cycles from other canisters (the caller).
        /// Other canisters can send cycles using `api::call::call_with_payment` method.
        /// Returns the actual amount of accepted cycles.
        #[update(name = "top_up")]
        async fn top_up() -> u64 {
            ic_helpers::management::Canister::accept_cycles()
        }

        /// Upgrades canisters and returns a list of outdated canisters.
        #[update(name = "upgrade")]
        async fn upgrade() -> Vec<candid::Principal> {
            candid::Principal::check_access($state::get().admin);
            $state::get().factory.upgrade($state::wasm()).await
        }

        /// Sets the admin principal.
        #[update(name = "set_admin")]
        async fn set_admin(admin: candid::Principal) {
            candid::Principal::check_access($state::get().admin);
            $state::get().admin = admin;
        }

        /// Returns the current version of canister.
        #[query(name = "version")]
        async fn version() -> String {
            env!("CARGO_PKG_VERSION").to_string()
        }

        /// Returns the length of canisters created by the factory.
        #[query(name = "length")]
        async fn length() -> usize {
            $state::get().factory.len()
        }

        /// Returns a vector of all canisters cretaed by the factory.
        #[query(name = "get_all")]
        async fn get_all() -> Vec<candid::Principal> {
            $state::get().factory.all()
        }
    };
}
