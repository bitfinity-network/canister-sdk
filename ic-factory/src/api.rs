use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use candid::Deserialize;
use ic_canister::{
    generate_exports, generate_idl, query, update, AsyncReturn, Canister, Idl, PreUpdate,
};
use ic_exports::candid::{CandidType, Nat, Principal};
use ic_exports::ic_base_types::PrincipalId;
use ic_exports::ic_cdk::export::candid::utils::ArgumentEncoder;
use ic_exports::ic_kit::ic;
use ic_exports::ledger::{AccountIdentifier, Subaccount, DEFAULT_TRANSFER_FEE};
use ic_helpers::ledger::LedgerPrincipalExt;
use ic_helpers::management::ManagementPrincipalExt;
use ic_storage::IcStorage;

use super::error::FactoryError;
use crate::{state, CmcConfig, INITIAL_CANISTER_CYCLES};

pub trait FactoryCanister: Canister + Sized + PreUpdate {
    fn cmc_config(&self) -> Rc<RefCell<CmcConfig>> {
        CmcConfig::get()
    }

    /// Returns the principal of CMC canister that the factory uses.
    #[query(trait = true)]
    fn cmc_principal(&self) -> Principal {
        self.cmc_config().borrow().cmc_principal()
    }

    /// Changes the CMC canister to use.
    ///
    /// Note, that real CMC canister can use only hard-coded principal due to protocol limitation.
    /// So this should only be used for testing with mock CMC canister.
    ///
    /// This method can only be called by the factory owner.
    #[update(trait = true)]
    fn set_cmc_principal(&mut self, cmc_principal: Principal) -> Result<(), FactoryError> {
        state::factory_state().check_is_owner()?;
        self.cmc_config().borrow_mut().cmc_principal = Some(cmc_principal);
        Ok(())
    }

    /// Returns the checksum of a wasm module in hex representation.
    #[query(trait = true)]
    fn get_checksum(&self) -> Result<String, FactoryError> {
        Ok(hex::encode(&state::factory_state().module()?.hash().0))
    }

    /// Returns the cycles balances.
    /// If principal == None then cycles balances of factory is returned,
    /// otherwise, cycles balances of `principal` is returned.
    /// If `principal` does not exists, `None` is returned.
    #[update(trait = true)]
    fn get_cycles(&self, principal: Option<Principal>) -> AsyncReturn<Option<Nat>> {
        let fut = async move {
            if let Some(principal) = principal {
                principal.status().await.map(|status| status.cycles).ok()
            } else {
                Some(ic_exports::ic_cdk::api::canister_balance().into())
            }
        };
        Box::pin(fut)
    }

    /// Accepts cycles from other canister.
    /// Other ic-helpers can send cycles using `api::call::call_with_payment` method.
    /// Returns the actual amount of accepted cycles.
    #[update(trait = true)]
    fn top_up(&self) -> u64 {
        <Principal as ManagementPrincipalExt>::accept_cycles()
    }

    fn set_canister_code(&self, wasm: Vec<u8>) -> Result<u32, FactoryError> {
        state::factory_state()
            .check_is_owner()?
            .set_canister_wasm(wasm)
    }

    #[allow(unused_variables)]
    #[allow(clippy::await_holding_refcell_ref)]
    fn create_canister<'a, T: ArgumentEncoder + Send + 'a>(
        &'a self,
        init_args: T,
        controller: Option<Principal>,
        caller: Option<Principal>,
    ) -> AsyncReturn<'a, Result<Principal, FactoryError>> {
        Box::pin(async move {
            let state_lock = state::factory_state().lock()?;

            let cycles_minted = {
                #[cfg(target_arch = "wasm32")]
                {
                    let caller = caller.unwrap_or_else(ic_exports::ic_kit::ic::caller);

                    state::factory_state()
                        .consume_provided_cycles_or_icp(caller, self.cmc_principal())
                        .await?
                }

                #[cfg(not(target_arch = "wasm32"))]
                {
                    0
                }
            };

            let cycles_to_canister = cycles_minted.min(INITIAL_CANISTER_CYCLES);

            let principal = state::factory_state()
                .create_canister(init_args, cycles_to_canister, &state_lock, controller)?
                .await
                .map_err(|e| FactoryError::CanisterCreateFailed(e.1))?;

            state::factory_state()
                .register_created(principal, &state_lock)
                .expect("correct state lock");

            Ok(principal)
        })
    }

    fn upgrade_canister(
        &mut self,
    ) -> AsyncReturn<Result<HashMap<Principal, UpgradeResult>, FactoryError>> {
        Box::pin(async move {
            let mut state = state::factory_state();
            let state_lock = state.lock()?;
            let caller = ic_exports::ic_kit::ic::caller();

            let canisters = state.canister_list();
            let module_hash = state.module()?.hash().0.clone();

            let mut results = HashMap::new();
            for canister in canisters {
                if state.canisters()[&canister].0 == module_hash {
                    results.insert(canister, UpgradeResult::Noop);
                    continue;
                }

                let upgrader = state
                    .check_is_owner_internal(caller)?
                    .upgrade(canister, &state_lock)?;

                let upgrade_result = match upgrader.await {
                    Ok(()) => UpgradeResult::Upgraded,
                    Err(e) => UpgradeResult::Error(e.1),
                };

                results.insert(canister, upgrade_result);
            }

            let mut owner = state.check_is_owner_internal(caller)?;
            for (canister, upgrade_result) in results.iter() {
                if matches!(upgrade_result, UpgradeResult::Upgraded) {
                    owner
                        .register_upgraded(*canister, &state_lock)
                        .expect("correct lock");
                }
            }

            Ok(results)
        })
    }

    #[update(trait = true)]
    fn reset_update_lock(&self) -> Result<(), FactoryError> {
        state::factory_state()
            .check_is_owner()?
            .release_update_lock();
        Ok(())
    }

    /// Returns the current version of canister.
    #[query(trait = true)]
    fn version(&self) -> Result<u32, FactoryError> {
        Ok(state::factory_state().module()?.version())
    }

    /// Returns the number of canisters created by the factory.
    #[query(trait = true)]
    fn length(&self) -> usize {
        state::factory_state().canister_count()
    }

    /// Returns a vector of all canisters created by the factory.
    #[query(trait = true)]
    fn get_all(&self) -> Vec<Principal> {
        state::factory_state().canister_list()
    }

    /// Returns the ICP fee amount for canister creation.
    #[query(trait = true)]
    fn get_icp_fee(&self) -> u64 {
        state::factory_state().icp_fee()
    }

    /// Sets the ICP fee amount for canister creation. This method can only be called
    /// by the factory controller.
    #[update(trait = true)]
    fn set_icp_fee(&self, e8s: u64) -> Result<(), FactoryError> {
        state::factory_state().check_is_owner()?.set_icp_fee(e8s)
    }

    /// Returns the principal that will receive the ICP fees.
    #[query(trait = true)]
    fn get_icp_to(&self) -> Principal {
        state::factory_state().icp_to()
    }

    /// Sets the principal that will receive the ICP fees. This method can only be called
    /// by the factory controller.
    #[update(trait = true)]
    fn set_icp_to(&self, to: Principal) -> Result<(), FactoryError> {
        state::factory_state().check_is_owner()?.set_fee_to(to)
    }

    /// Returns the ICPs transferred to the factory by the caller. This method returns all
    /// not used ICP minus transaction fee.
    #[update(trait = true)]
    fn refund_icp(&self) -> AsyncReturn<Result<u64, FactoryError>> {
        let ledger = state::factory_state().ledger_principal();
        Box::pin(async move {
            let caller = ic_exports::ic_kit::ic::caller();
            let balance = ledger
                .get_balance(
                    ic_exports::ic_kit::ic::id(),
                    Some(Subaccount::from(&PrincipalId(caller))),
                )
                .await
                .map_err(FactoryError::LedgerError)?;

            if balance < DEFAULT_TRANSFER_FEE.get_e8s() {
                // Nothing to refund
                return Ok(0);
            }

            ledger
                .transfer(
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
        state::factory_state()
            .check_is_owner()?
            .set_controller(controller)
    }

    /// Returns the factory controller principal.
    #[query(trait = true)]
    fn get_controller(&self) -> Principal {
        state::factory_state().controller()
    }

    /// Returns the AccountIdentifier for the caller subaccount in the factory account.
    #[query(trait = true)]
    fn get_ledger_account_id(&self) -> String {
        let factory_id = ic::id();
        let caller = ic::caller();
        let account = AccountIdentifier::new(
            PrincipalId(factory_id),
            Some(Subaccount::from(&PrincipalId(caller))),
        );

        account.to_hex()
    }

    // Important: This function *must* be defined to be the
    // last one in the trait because it depends on the order
    // of expansion of update/query(trait = true) methods.
    fn get_idl() -> Idl {
        generate_idl!()
    }

    #[allow(clippy::await_holding_refcell_ref)]
    fn drop_canister(
        &self,
        canister_id: Principal,
        caller: Option<Principal>,
    ) -> AsyncReturn<Result<(), FactoryError>> {
        Box::pin(async move {
            let state_lock = state::factory_state().lock()?;
            let caller = caller.unwrap_or_else(ic_exports::ic_kit::ic::caller);

            state::factory_state()
                .check_is_owner_internal(caller)?
                .drop_canister(canister_id, &state_lock)
                .await?;

            state::factory_state()
                .check_is_owner_internal(caller)?
                .register_dropped(canister_id, &state_lock)
        })
    }
}

#[derive(Debug, Deserialize, CandidType)]
pub enum UpgradeResult {
    Noop,
    Upgraded,
    Error(String),
}

generate_exports!(FactoryCanister);
