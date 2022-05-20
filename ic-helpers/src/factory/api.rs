/// This macro adds some common API method for a factory canister. For it to work properly:
/// * $state type must implement `ic_helpers::factory::FactoryState` and `ic_storage::IcStorage`
///   traits, and this traits must be in the scope of the macro invocation
/// * the calling create must have `ic_cdk` create in the dependencies
/// * the calling crate must have `dfn-core` and `ledger-canister` crates in the dependencies. This
///   crates are found in the `infinity-swap/ic` repo.
/// * these types must be in scope of the macro invocation: `candid::{Nat, Principal, candid_type}`,
///   `ic_cdk_macros::{query, update}`. (Unfortunately, if we redo these imports inside the macro,
///   `candid_type` functionality does not work properly, so these imports must be taken care
///   of manually)
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
macro_rules! extend_with_factory_api {
    ( $canister:ident, $state:ident, $upgrading_bytecode:expr ) => {
        impl $canister {
            /// Returns the checksum of a wasm module in hex representation.
            #[query]
            fn get_checksum(&self) -> String {
                self.$state.borrow().factory().checksum.to_string()
            }

            /// Returns the cycles balances.
            /// If principal == None then cycles balances of factory is returned,
            /// otherwise, cycles balances of `principal` is returned.
            /// If `principal` does not exists, `None` is returned.
            #[update]
            async fn get_cycles(&self, principal: Option<Principal>) -> Option<Nat> {
                Some(if let Some(principal) = principal {
                    ::ic_helpers::management::Canister::from(principal)
                        .status()
                        .await
                        .map(|status| status.cycles)
                        .ok()?
                } else {
                    ::ic_cdk::api::canister_balance().into()
                })
            }

            /// Accepts cycles from other canister.
            /// Other ic-helpers can send cycles using `api::call::call_with_payment` method.
            /// Returns the actual amount of accepted cycles.
            #[update]
            fn top_up(&self) -> u64 {
                ::ic_helpers::management::Canister::accept_cycles()
            }

            /// Upgrades canisters controller by the factory and returns a list of outdated canisters
            /// (in case an upgrade error occurs).
            #[update]
            async fn upgrade(&self) -> Vec<Principal> {
                // TODO: At the moment we do not do any security checks for this method, for even if there's
                // nothing to upgrade, it will just check all ic-helpers and do nothing else.
                // Later, we should add here (and in create_canister methods) a cycle check,
                // to make the caller to pay for the execution of this method.

                let canisters = self.$state.borrow().factory().canisters.clone();
                let curr_version = self.$state.borrow().factory().checksum.version;
                let mut outdated_canisters = vec![];

                for (key, canister) in canisters
                    .into_iter()
                    .filter(|(_, c)| c.version() == curr_version)
                {
                    let upgrader = self
                        .$state
                        .borrow()
                        .factory()
                        .upgrade(&canister, $upgrading_bytecode);
                    match upgrader.await {
                        Ok(upgraded) => self
                            .$state
                            .borrow_mut()
                            .factory_mut()
                            .register_upgraded(&key, upgraded),
                        Err(_) => outdated_canisters.push(canister.identity()),
                    }
                }

                outdated_canisters
            }

            /// Returns the current version of canister.
            #[query]
            fn version(&self) -> &'static str {
                env!("CARGO_PKG_VERSION")
            }

            /// Returns the number of canisters created by the factory.
            #[query]
            fn length(&self) -> usize {
                self.$state.borrow().factory().len()
            }

            /// Returns a vector of all canisters created by the factory.
            #[query]
            fn get_all(&self) -> Vec<Principal> {
                self.$state.borrow().factory().all()
            }

            /// Returns the ICP fee amount for canister creation.
            #[query]
            fn get_icp_fee(&self) -> u64 {
                self.$state.borrow().icp_fee()
            }

            /// Sets the ICP fee amount for canister creation. This method can only be called
            /// by the factory controller.
            #[update]
            fn set_icp_fee(&self, e8s: u64) -> ::std::result::Result<(), FactoryError> {
                self.$state.borrow_mut().set_icp_fee(e8s)
            }

            /// Returns the principal that will receive the ICP fees.
            #[query]
            fn get_icp_to(&self) -> Principal {
                self.$state.borrow().icp_to()
            }

            /// Sets the principal that will receive the ICP fees. This method can only be called
            /// by the factory controller.
            #[update]
            fn set_icp_to(&self, to: Principal) -> ::std::result::Result<(), FactoryError> {
                self.$state.borrow_mut().set_icp_to(to)
            }

            /// Returns the ICPs transferred to the factory by the caller. This method returns all
            /// not used ICP minus transaction fee.
            #[update]
            async fn refund_icp(&self) -> ::std::result::Result<u64, FactoryError> {
                use ::ic_helpers::ledger::{
                    LedgerPrincipalExt, PrincipalId, Subaccount, DEFAULT_TRANSFER_FEE,
                };

                let caller = ::ic_cdk::caller();
                let ledger = self.$state.borrow().ledger_principal();
                let balance = ledger
                    .get_balance(
                        ::ic_cdk::api::id(),
                        Some(Subaccount::from(&PrincipalId(caller))),
                    )
                    .await
                    .map_err(|e| FactoryError::LedgerError(e))?;

                if balance < DEFAULT_TRANSFER_FEE.get_e8s() {
                    // Nothing to refund
                    return Ok(0);
                }

                LedgerPrincipalExt::transfer(
                    &ledger,
                    caller,
                    balance,
                    Some(Subaccount::from(&PrincipalId(caller))),
                    None,
                )
                .await
                .map_err(|e| FactoryError::LedgerError(e))
            }

            /// Sets the factory controller principal.
            #[update]
            fn set_controller(
                &self,
                controller: Principal,
            ) -> ::std::result::Result<(), FactoryError> {
                self.$state.borrow_mut().set_controller(controller)
            }

            /// Returns the factory controller principal.
            #[query]
            fn get_controller(&self) -> Principal {
                self.$state.borrow().controller()
            }

            /// Returns the AccountIdentifier for the caller subaccount in the factory account.
            #[query]
            fn get_ledger_account_id(&self) -> String {
                use ::ic_helpers::ledger::{AccountIdentifier, PrincipalId, Subaccount};

                let factory_id = ::ic_cdk::api::id();
                let caller = ::ic_cdk::api::caller();
                let account = AccountIdentifier::new(
                    PrincipalId(factory_id),
                    Some(Subaccount::from(&PrincipalId(caller))),
                );

                account.to_hex()
            }
        }
    };
}
