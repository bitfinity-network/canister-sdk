use std::{cell::RefCell, rc::Rc};

use candid::{Nat, Principal};
use ic_canister::{generate_exports, query, update, AsyncReturn, Canister};

use ic_helpers::management;

use super::{error::FactoryError, FactoryState};

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
pub trait FactoryCanister: Canister + Sized {
    fn state(&self) -> Rc<RefCell<FactoryState>>;

    fn get_canister_bytecode() -> Vec<u8>;

    /// Returns the checksum of a wasm module in hex representation.
    #[query(trait = true)]
    fn get_checksum<'a>(&'a self) -> String {
        self.state().borrow().factory.checksum.to_string()
    }

    /// Returns the cycles balances.
    /// If principal == None then cycles balances of factory is returned,
    /// otherwise, cycles balances of `principal` is returned.
    /// If `principal` does not exists, `None` is returned.
    #[update(trait = true)]
    fn get_cycles<'a>(&'a self, principal: Option<Principal>) -> AsyncReturn<Option<Nat>> {
        let fut = async move {
            if let Some(principal) = principal {
                management::Canister::from(principal)
                    .status()
                    .await
                    .map(|status| status.cycles)
                    .ok()
            } else {
                Some(ic_cdk::api::canister_balance().into())
            }
        };
        Box::pin(fut)
    }

    /// Accepts cycles from other canister.
    /// Other ic-helpers can send cycles using `api::call::call_with_payment` method.
    /// Returns the actual amount of accepted cycles.
    #[update(trait = true)]
    fn top_up(&self) -> u64 {
        management::Canister::accept_cycles()
    }

    /// Upgrades canisters controller by the factory and returns a list of outdated canisters
    /// (in case an upgrade error occurs).
    #[update(trait = true)]
    fn upgrade<'a>(&'a mut self) -> AsyncReturn<Vec<Principal>> {
        // TODO: At the moment we do not do any security checks for this method, for even if there's
        // nothing to upgrade, it will just check all ic-helpers and do nothing else.
        // Later, we should add here (and in create_canister methods) a cycle check,
        // to make the caller to pay for the execution of this method.

        let canister_bytecode = <Self as FactoryCanister>::get_canister_bytecode();
        Box::pin(async move {
            let canisters = self.state().borrow_mut().factory.canisters.clone();
            let curr_version = self.state().borrow().factory.checksum.version;
            let mut outdated_canisters = vec![];

            for (key, canister) in canisters
                .into_iter()
                .filter(|(_, c)| c.version() == curr_version)
            {
                let upgrader = self
                    .state()
                    .borrow_mut()
                    .factory
                    .upgrade(&canister, canister_bytecode.clone());
                if let Ok(upgraded) = upgrader.await {
                    self.state()
                        .borrow_mut()
                        .factory
                        .register_upgraded(&key, upgraded)
                } else {
                    outdated_canisters.push(canister.identity())
                }
            }

            outdated_canisters
        })
    }

    /// Returns the current version of canister.
    #[query(trait = true)]
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// Returns the number of canisters created by the factory.
    #[query(trait = true)]
    fn length(&self) -> usize {
        self.state().borrow().factory.canisters.len()
    }

    /// Returns a vector of all canisters created by the factory.
    #[query(trait = true)]
    fn get_all(&self) -> Vec<Principal> {
        self.state().borrow().factory.all()
    }

    /// Returns the ICP fee amount for canister creation.
    #[query(trait = true)]
    fn get_icp_fee(&self) -> u64 {
        self.state().borrow().icp_fee()
    }

    /// Sets the ICP fee amount for canister creation. This method can only be called
    /// by the factory controller.
    #[update(trait = true)]
    fn set_icp_fee(&self, e8s: u64) -> Result<(), FactoryError> {
        self.state().borrow_mut().set_icp_fee(e8s)
    }

    /// Returns the principal that will receive the ICP fees.
    #[query(trait = true)]
    fn get_icp_to(&self) -> Principal {
        self.state().borrow().icp_to()
    }

    /// Sets the principal that will receive the ICP fees. This method can only be called
    /// by the factory controller.
    #[update(trait = true)]
    fn set_icp_to(&self, to: Principal) -> ::std::result::Result<(), FactoryError> {
        self.state().borrow_mut().set_icp_to(to)
    }

    /// Returns the ICPs transferred to the factory by the caller. This method returns all
    /// not used ICP minus transaction fee.
    #[update(trait = true)]
    fn refund_icp<'a>(&'a self) -> AsyncReturn<'a, Result<u64, FactoryError>> {
        use ic_helpers::ledger::{
            LedgerPrincipalExt, PrincipalId, Subaccount, DEFAULT_TRANSFER_FEE,
        };

        let ledger = self.state().borrow().ledger_principal();
        Box::pin(async move {
            let caller = ic_kit::ic::caller();
            let balance = ledger
                .get_balance(
                    ic_kit::ic::id(),
                    Some(Subaccount::from(&PrincipalId(caller))),
                )
                .await
                .map_err(FactoryError::LedgerError)?;

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
            .map_err(FactoryError::LedgerError)
        })
    }

    /// Sets the factory controller principal.
    #[update(trait = true)]
    fn set_controller(&self, controller: Principal) -> Result<(), FactoryError> {
        self.state().borrow_mut().set_controller(controller)
    }

    /// Returns the factory controller principal.
    #[query(trait = true)]
    fn get_controller(&self) -> Principal {
        self.state().borrow().controller()
    }

    /// Returns the AccountIdentifier for the caller subaccount in the factory account.
    #[query(trait = true)]
    fn get_ledger_account_id(&self) -> String {
        use ic_helpers::ledger::{AccountIdentifier, PrincipalId, Subaccount};

        let factory_id = ic_kit::ic::id();
        let caller = ic_kit::ic::caller();
        let account = AccountIdentifier::new(
            PrincipalId(factory_id),
            Some(Subaccount::from(&PrincipalId(caller))),
        );

        account.to_hex()
    }

    // Important: This function *must* be defined to be the
    // last one in the trait because it depends on the order
    // of expansion of update/query(trait = true) methods.
    fn get_idl() -> ic_canister::Idl {
        ic_canister::generate_idl!()
    }
}

#[derive(Clone, Canister)]
pub struct FactoryExport {
    #[id]
    principal: Principal,
    #[state(stable_store = false)]
    state: Rc<RefCell<FactoryState>>,
}

const FACTORY_WASM_PATH: &str = "../../target/wasm32-unknown-unknown/release/factory.wasm";

impl FactoryCanister for FactoryExport {
    fn get_canister_bytecode() -> Vec<u8> {
        ic_helpers::get_canister_bytecode_for(FACTORY_WASM_PATH)
    }

    fn state(&self) -> Rc<RefCell<FactoryState>> {
        self.state.clone()
    }
}

generate_exports!(FactoryExport);
