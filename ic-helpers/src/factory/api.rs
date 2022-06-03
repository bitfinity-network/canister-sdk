use std::{
    cell::{Ref, RefMut},
    future::Future,
    pin::Pin,
};

use candid::{Nat, Principal};
use ic_canister::{query, update, Canister};

use crate::management;

use super::{error::FactoryError, FactoryState};

// Important: If you're renaming this type, don't forget to update
// the `ic_canister_macros::api::get_args` as well.
pub type AsyncReturn<T> = Pin<Box<dyn Future<Output = T> + Send>>;

pub trait FactoryCanisterKeyBounds: std::hash::Hash + Eq + Clone + Send + 'static {}
impl<T> FactoryCanisterKeyBounds for T where T: std::hash::Hash + Eq + Clone + Send + 'static {}

pub trait CanisterState<
    CanisterStateStruct: FactoryState<FactoryCanisterKey>,
    FactoryCanisterKey: FactoryCanisterKeyBounds,
>
{
    fn state(&self) -> Ref<'_, CanisterStateStruct>;
    fn state_mut(&self) -> RefMut<'_, CanisterStateStruct>;

    // TODO: return reference
    fn factory(&self) -> super::Factory<FactoryCanisterKey>;
    fn factory_mut(&mut self) -> super::Factory<FactoryCanisterKey>;
}

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
pub trait FactoryCanister<CanisterStateStruct: FactoryState<Self::FactoryCanisterKey>>:
    Canister + CanisterState<CanisterStateStruct, Self::FactoryCanisterKey>
{
    type FactoryCanisterKey: FactoryCanisterKeyBounds;

    fn get_canister_bytecode() -> Vec<u8>;

    /// Returns the checksum of a wasm module in hex representation.
    #[query]
    fn get_checksum<'a>(&'a self) -> String {
        self.state().checksum().to_string()
    }

    /// Returns the cycles balances.
    /// If principal == None then cycles balances of factory is returned,
    /// otherwise, cycles balances of `principal` is returned.
    /// If `principal` does not exists, `None` is returned.
    #[update]
    fn get_cycles(&self, principal: Option<Principal>) -> AsyncReturn<Option<Nat>> {
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
    #[update]
    fn top_up(&self) -> u64 {
        management::Canister::accept_cycles()
    }

    /// Upgrades canisters controller by the factory and returns a list of outdated canisters
    /// (in case an upgrade error occurs).
    // #[update]
    // fn upgrade(&mut self) -> AsyncReturn<Vec<Principal>> {
    #[update]
    fn upgrade(&mut self) -> AsyncReturn<Vec<Principal>> {
        // TODO: At the moment we do not do any security checks for this method, for even if there's
        // nothing to upgrade, it will just check all ic-helpers and do nothing else.
        // Later, we should add here (and in create_canister methods) a cycle check,
        // to make the caller to pay for the execution of this method.

        let mut factory = self.factory_mut();
        let canister_bytecode =
            <Self as FactoryCanister<CanisterStateStruct>>::get_canister_bytecode();
        Box::pin(async move {
            let canisters = factory.canisters.clone();
            let curr_version = factory.checksum.version;
            let mut outdated_canisters = vec![];

            for (key, canister) in canisters
                .into_iter()
                .filter(|(_, c)| c.version() == curr_version)
            {
                let upgrader = factory.upgrade(&canister, canister_bytecode.clone());
                match upgrader.await {
                    Ok(upgraded) => factory.register_upgraded(&key, upgraded),
                    Err(_) => outdated_canisters.push(canister.identity()),
                }
            }

            outdated_canisters
        })
    }

    /// Returns the current version of canister.
    #[query]
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// Returns the number of canisters created by the factory.
    #[query]
    fn length(&self) -> usize {
        self.factory().canisters.len()
    }

    /// Returns a vector of all canisters created by the factory.
    #[query]
    fn get_all(&self) -> Vec<Principal> {
        self.factory().all()
    }

    /// Returns the ICP fee amount for canister creation.
    #[query]
    fn get_icp_fee(&self) -> u64 {
        self.state().icp_fee()
    }

    /// Sets the ICP fee amount for canister creation. This method can only be called
    /// by the factory controller.
    #[update]
    fn set_icp_fee(&self, e8s: u64) -> Result<(), FactoryError> {
        self.state_mut().set_icp_fee(e8s)
    }

    /// Returns the principal that will receive the ICP fees.
    #[query]
    fn get_icp_to(&self) -> Principal {
        self.state().icp_to()
    }

    /// Sets the principal that will receive the ICP fees. This method can only be called
    /// by the factory controller.
    #[update]
    fn set_icp_to(&self, to: Principal) -> ::std::result::Result<(), FactoryError> {
        self.state_mut().set_icp_to(to)
    }

    /// Returns the ICPs transferred to the factory by the caller. This method returns all
    /// not used ICP minus transaction fee.
    #[update]
    fn refund_icp(&self) -> AsyncReturn<Result<u64, FactoryError>> {
        use crate::ledger::{LedgerPrincipalExt, PrincipalId, Subaccount, DEFAULT_TRANSFER_FEE};

        let ledger = self.state().ledger_principal();
        Box::pin(async move {
            let caller = ic_kit::ic::caller();
            let balance = ledger
                .get_balance(
                    ic_kit::ic::id(),
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
        })
    }

    /// Sets the factory controller principal.
    #[update]
    fn set_controller(&self, controller: Principal) -> Result<(), FactoryError> {
        self.state_mut().set_controller(controller)
    }

    /// Returns the factory controller principal.
    #[query]
    fn get_controller(&self) -> Principal {
        self.state().controller()
    }

    /// Returns the AccountIdentifier for the caller subaccount in the factory account.
    #[query]
    fn get_ledger_account_id(&self) -> String {
        use crate::ledger::{AccountIdentifier, PrincipalId, Subaccount};

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
    // of expansion of update/query methods.
    fn get_idl() -> ic_canister::Idl {
        ic_canister::generate_idl!()
    }
}
