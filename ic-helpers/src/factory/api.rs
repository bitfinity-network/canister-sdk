/// This macro adds some common API method for a factory canister. For it to work properly:
/// * $state type must implement `ic_helpers::factory::FactoryState` and `ic_storage::IcStorage`
///   traits
/// * the calling crate must have `ic_types` and `ledger_canister` crates in the dependencies. This
///   crates are found in the `dfinity/ic` repo.
///
/// API methods that are added:
/// * get_checksum
/// * get_cycles
/// * top_up
/// * upgrade
/// * version
/// * length
/// * get_all
/// * get_icp_fee
/// * set_icp_fee
/// * get_icp_to
/// * set_icp_to
/// * get_controller
/// * set_controller
/// * refund_icp
#[macro_export]
macro_rules! init_factory_api {
    ( $state:ident, $bytecode:expr ) => {
        // Add this block not to pollute caller context with our use.
        mod __factory_api {
            use super::$state;
            use ::ic_helpers::is20::IS20PrincipalExt;
            use ::ic_helpers::ledger::LedgerPrincipalExt;
            use ::ic_storage::IcStorage;
            use ::ledger_canister::{Subaccount, TRANSACTION_FEE};
            use ::ic_types::PrincipalId;
            use ::ic_cdk_macros::{update, query};
            use ::ic_cdk::export::candid::{Principal, candid_method, Nat};
            use ::ic_helpers::factory::error::FactoryError;
            use ::ic_helpers::factory::FactoryState;

            /// Returns the checksum of a wasm module in hex representation.
            #[query(name = "get_checksum")]
            #[candid_method(query, rename = "get_checksum")]
            async fn get_checksum() -> String {
                $state::get().borrow().factory().checksum.to_string()
            }

            /// Returns the cycles balances.
            /// If principal == None then cycles balances of factory is returned,
            /// otherwise, cycles balances of `principal` is returned.
            /// If `principal` does not exists, `None` is returned.
            #[update(name = "get_cycles")]
            #[candid_method(update, rename = "get_cycles")]
            async fn get_cycles(principal: Option<Principal>) -> Option<Nat> {
                Some(if let Some(principal) = principal {
                    ::ic_helpers::management::Canister::from(principal)
                        .status()
                        .await
                        .map(|status| status.cycles)
                        .ok()?
                } else {
                    Principal::cycles()
                })
            }

            /// Accepts cycles from other ic-helpers (the caller).
            /// Other ic-helpers can send cycles using `api::call::call_with_payment` method.
            /// Returns the actual amount of accepted cycles.
            #[update(name = "top_up")]
            #[candid_method(update, rename = "top_up")]
            async fn top_up() -> u64 {
                ::ic_helpers::management::Canister::accept_cycles()
            }

            /// Upgrades ic-helpers and returns a list of outdated ic-helpers.
            #[update(name = "upgrade")]
            #[candid_method(update, rename = "upgrade")]
            async fn upgrade() -> Vec<Principal> {
                // TODO: At the moment we do not do any security checks for this method, for even if there's
                // nothing to upgrade, it will just check all ic-helpers and do nothing else.
                // Later, we should add here (and in create_canister methods) a cycle check,
                // to make the caller to pay for the execution of this method.

                let state = $state::get();
                let canisters = state.borrow().factory().canisters.clone();
                let curr_version = state.borrow().factory().checksum.version;
                let mut outdated_canisters = vec![];

                for (key, canister) in canisters {
                    if canister.version() == curr_version {
                        continue;
                    }

                    let upgrader = state.borrow().factory().upgrade(&canister, $bytecode);
                    match upgrader.await {
                        Ok(upgraded) => {
                            state.borrow_mut().factory_mut().register_upgraded(&key, upgraded)
                        }
                        Err(_) => outdated_canisters.push(canister.identity()),
                    }
                }

                outdated_canisters
            }

            /// Returns the current version of canister.
            #[query(name = "version")]
            #[candid_method(query, rename = "version")]
            async fn version() -> String {
                env!("CARGO_PKG_VERSION").to_string()
            }

            /// Returns the length of ic-helpers created by the factory.
            #[query(name = "length")]
            #[candid_method(query, rename = "length")]
            async fn length() -> usize {
                $state::get().borrow().factory().len()
            }

            /// Returns a vector of all ic-helpers created by the factory.
            #[query(name = "get_all")]
            #[candid_method(query, rename = "get_all")]
            async fn get_all() -> Vec<Principal> {
                $state::get().borrow().factory().all()
            }

            /// Returns the ICP fee amount for canister creation.
            #[query]
            #[candid_method(query)]
            pub fn get_icp_fee() -> u64 {
                State::get().borrow().icp_fee()
            }

            /// Sets the ICP fee amount for canister creation. This method can only be called
            /// by the factory controller.
            #[update]
            #[candid_method(update)]
            pub fn set_icp_fee(e8s: u64) {
                State::get().borrow_mut().set_icp_fee(e8s);
            }

            /// Returns the principal that will receive the ICP fees.
            #[query]
            #[candid_method(query)]
            pub fn get_icp_to() -> Principal {
                State::get().borrow().icp_to()
            }

            /// Sets the principal that will receive the ICP fees. This method can only be called
            /// by the factory controller.
            #[update]
            #[candid_method(update)]
            pub fn set_icp_to(to: Principal) {
                Principal::check_access(State::get().borrow().controller);
                State::get().borrow_mut().set_icp_to(to);
            }

            /// Returns the ICPs transferred to the factory by the caller. This method returns all
            /// not used ICP minus transaction fee.
            #[update]
            #[candid_method(update)]
            pub async fn refund_icp() -> Result<u64, FactoryError> {
                let caller = ic_cdk::caller();
                let ledger = State::get().borrow().ledger_principal();
                let balance = ledger
                    .get_balance(
                        ic_kit::ic::id(),
                        Some(Subaccount::from(&PrincipalId::from(caller))),
                    )
                    .await
                    .map_err(|e| FactoryError::LedgerError(e))?;

                if balance < TRANSACTION_FEE.get_e8s() {
                    // Nothing to refund
                    return Ok(0);
                }

                LedgerPrincipalExt::transfer(
                    &ledger,
                    caller,
                    balance,
                    Some(Subaccount::from(&PrincipalId::from(caller))),
                    None,
                )
                .await
                .map_err(|e| FactoryError::LedgerError(e))
            }

            /// Sets the principal that can set `fee_to` principal and configure liquidity caps.
            #[update(name = "set_controller")]
            #[candid_method(update, rename = "set_controller")]
            fn set_controller(controller: Principal) {
                State::get().borrow_mut().set_controller(controller);
            }

            /// Returns the principal that can set `fee_to` principal and configure liquidity caps.
            #[query(name = "get_controller")]
            #[candid_method(query, rename = "get_controller")]
            fn get_controller() -> Principal {
                State::get().borrow().controller()
            }
        }
    };
}
