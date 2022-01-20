#[macro_export]
macro_rules! init_factory_api {
    ( $state:ident, $bytecode:expr ) => {
        // Add this block not to pollute caller context with our use.
        mod __factory_api {
            use super::$state;
            use ::ic_helpers::is20::IS20PrincipalExt;
            use ::ic_storage::IcStorage;

            /// Returns the checksum of a wasm module in hex representation.
            #[::ic_cdk_macros::query(name = "get_checksum")]
            #[::candid::candid_method(query, rename = "get_checksum")]
            async fn get_checksum() -> String {
                $state::get().borrow().factory.checksum.to_string()
            }

            /// Returns the cycles balances.
            /// If principal == None then cycles balances of factory is returned,
            /// otherwise, cycles balances of `principal` is returned.
            /// If `principal` does not exists, `None` is returned.
            #[::ic_cdk_macros::update(name = "get_cycles")]
            #[::candid::candid_method(update, rename = "get_cycles")]
            async fn get_cycles(principal: Option<::candid::Principal>) -> Option<::candid::Nat> {
                Some(if let Some(principal) = principal {
                    ::ic_helpers::management::Canister::from(principal)
                        .status()
                        .await
                        .map(|status| status.cycles)
                        .ok()?
                } else {
                    ::candid::Principal::cycles()
                })
            }

            /// Accepts cycles from other ic-helpers (the caller).
            /// Other ic-helpers can send cycles using `api::call::call_with_payment` method.
            /// Returns the actual amount of accepted cycles.
            #[::ic_cdk_macros::update(name = "top_up")]
            #[::candid::candid_method(update, rename = "top_up")]
            async fn top_up() -> u64 {
                ::ic_helpers::management::Canister::accept_cycles()
            }

            /// Upgrades ic-helpers and returns a list of outdated ic-helpers.
            #[::ic_cdk_macros::update(name = "upgrade")]
            #[::candid::candid_method(update, rename = "upgrade")]
            async fn upgrade() -> Vec<::candid::Principal> {
                // TODO: At the moment we do not do any security checks for this method, for even if there's
                // nothing to upgrade, it will just check all ic-helpers and do nothing else.
                // Later, we should add here (and in create_canister methods) a cycle check,
                // to make the caller to pay for the execution of this method.

                let state = $state::get();
                let canisters = state.borrow().factory.canisters.clone();
                let curr_version = state.borrow().factory.checksum.version;
                let mut outdated_canisters = vec![];

                for (key, canister) in canisters {
                    if canister.version() == curr_version {
                        continue;
                    }

                    let upgrader = state.borrow().factory.upgrade(&canister, $bytecode);
                    match upgrader.await {
                        Ok(upgraded) => {
                            state.borrow_mut().factory.register_upgraded(&key, upgraded)
                        }
                        Err(_) => outdated_canisters.push(canister.identity()),
                    }
                }

                outdated_canisters
            }

            /// Returns the current version of canister.
            #[::ic_cdk_macros::query(name = "version")]
            #[::candid::candid_method(query, rename = "version")]
            async fn version() -> String {
                env!("CARGO_PKG_VERSION").to_string()
            }

            /// Returns the length of ic-helpers created by the factory.
            #[::ic_cdk_macros::query(name = "length")]
            #[::candid::candid_method(query, rename = "length")]
            async fn length() -> usize {
                $state::get().borrow().factory.len()
            }

            /// Returns a vector of all ic-helpers cretaed by the factory.
            #[::ic_cdk_macros::query(name = "get_all")]
            #[::candid::candid_method(query, rename = "get_all")]
            async fn get_all() -> Vec<candid::Principal> {
                $state::get().borrow().factory.all()
            }
        }
    };
}
