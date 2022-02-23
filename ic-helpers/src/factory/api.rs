/// This macro adds some common API method for a factory canister. For it to work properly:
/// * $state type must implement `ic_helpers::factory::FactoryState` and `ic_storage::IcStorage`
///   traits, and this traits must be in the scope of the macro invocation
/// * the calling create must have `ic_cdk` create in the dependencies
/// * the calling crate must have `ic_types` and `ledger_canister` crates in the dependencies. This
///   crates are found in the `dfinity/ic` repo.
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
macro_rules! init_factory_api {
    ( $state:ident, $bytecode:expr ) => {
        /// Returns the checksum of a wasm module in hex representation.
        #[query(name = "get_checksum")]
        #[candid_method(query, rename = "get_checksum")]
        pub fn get_checksum() -> String {
            $state::get().borrow().factory().checksum.to_string()
        }

        /// Returns the cycles balances.
        /// If principal == None then cycles balances of factory is returned,
        /// otherwise, cycles balances of `principal` is returned.
        /// If `principal` does not exists, `None` is returned.
        #[update(name = "get_cycles")]
        #[candid_method(update, rename = "get_cycles")]
        pub async fn get_cycles(principal: Option<Principal>) -> Option<Nat> {
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
        #[update(name = "top_up")]
        #[candid_method(update, rename = "top_up")]
        pub fn top_up() -> u64 {
            ::ic_helpers::management::Canister::accept_cycles()
        }

        /// Upgrades canisters controller by the factory and returns a list of outdated canisters
        /// (in case an upgrade error occurs).
        #[update(name = "upgrade")]
        #[candid_method(update, rename = "upgrade")]
        pub async fn upgrade() -> Vec<Principal> {
            // TODO: At the moment we do not do any security checks for this method, for even if there's
            // nothing to upgrade, it will just check all ic-helpers and do nothing else.
            // Later, we should add here (and in create_canister methods) a cycle check,
            // to make the caller to pay for the execution of this method.

            let state = $state::get();
            let canisters = state.borrow().factory().canisters.clone();
            let curr_version = state.borrow().factory().checksum.version;
            let mut outdated_canisters = vec![];

            for (key, canister) in canisters
                .into_iter()
                .filter(|(_, c)| c.version() == curr_version)
            {
                let upgrader = state.borrow().factory().upgrade(&canister, $bytecode);
                match upgrader.await {
                    Ok(upgraded) => state
                        .borrow_mut()
                        .factory_mut()
                        .register_upgraded(&key, upgraded),
                    Err(_) => outdated_canisters.push(canister.identity()),
                }
            }

            outdated_canisters
        }

        /// Returns the current version of canister.
        #[query(name = "version")]
        #[candid_method(query, rename = "version")]
        pub fn version() -> &'static str {
            env!("CARGO_PKG_VERSION")
        }

        /// Returns the number of canisters created by the factory.
        #[query(name = "length")]
        #[candid_method(query, rename = "length")]
        pub fn length() -> usize {
            $state::get().borrow().factory().len()
        }

        /// Returns a vector of all canisters created by the factory.
        #[query(name = "get_all")]
        #[candid_method(query, rename = "get_all")]
        pub fn get_all() -> Vec<Principal> {
            $state::get().borrow().factory().all()
        }

        /// Returns the ICP fee amount for canister creation.
        #[query]
        #[candid_method(query)]
        pub fn get_icp_fee() -> u64 {
            $state::get().borrow().icp_fee()
        }

        /// Sets the ICP fee amount for canister creation. This method can only be called
        /// by the factory controller.
        #[update]
        #[candid_method(update)]
        pub fn set_icp_fee(e8s: u64) -> ::std::result::Result<(), FactoryError> {
            $state::get().borrow_mut().set_icp_fee(e8s)
        }

        /// Returns the principal that will receive the ICP fees.
        #[query]
        #[candid_method(query)]
        pub fn get_icp_to() -> Principal {
            $state::get().borrow().icp_to()
        }

        /// Sets the principal that will receive the ICP fees. This method can only be called
        /// by the factory controller.
        #[update]
        #[candid_method(update)]
        pub fn set_icp_to(to: Principal) -> ::std::result::Result<(), FactoryError> {
            $state::get().borrow_mut().set_icp_to(to)
        }

        /// Returns the ICPs transferred to the factory by the caller. This method returns all
        /// not used ICP minus transaction fee.
        #[update]
        #[candid_method(update)]
        pub async fn refund_icp() -> ::std::result::Result<u64, FactoryError> {
            use ::ic_helpers::ledger::LedgerPrincipalExt;
            use ::ic_types::PrincipalId;
            use ::ledger_canister::{Subaccount, TRANSACTION_FEE};

            let caller = ::ic_cdk::caller();
            let ledger = $state::get().borrow().ledger_principal();
            let balance = ledger
                .get_balance(
                    ::ic_cdk::api::id(),
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

        /// Sets the factory controller principal.
        #[update(name = "set_controller")]
        #[candid_method(update, rename = "set_controller")]
        fn set_controller(controller: Principal) -> ::std::result::Result<(), FactoryError> {
            $state::get().borrow_mut().set_controller(controller)
        }

        /// Returns the factory controller principal.
        #[query(name = "get_controller")]
        #[candid_method(query, rename = "get_controller")]
        fn get_controller() -> Principal {
            $state::get().borrow().controller()
        }

        /// Returns the AccountIdentifier for the caller subaccount in the factory account.
        #[query]
        #[candid_method(query)]
        fn get_ledger_account_id() -> String {
            use ::ic_helpers::ledger::FromPrincipal;
            use ::ic_types::PrincipalId;
            use ::ledger_canister::{account_identifier::AccountIdentifier, Subaccount};

            let factory_id = ::ic_cdk::api::id();
            let caller = ::ic_cdk::api::caller();
            let subaccount = Subaccount::from(&PrincipalId::from(caller));
            let account = AccountIdentifier::from_principal(factory_id, Some(subaccount));

            account.to_hex()
        }
    };
}
