use super::{error::FactoryError, FactoryState};
use candid::{CandidType, Nat, Principal};
use ic_canister::{
    generate_exports, query, state_getter, update, virtual_canister_call, AsyncReturn, Canister,
    PreUpdate,
};
use ic_cdk::export::candid::utils::ArgumentEncoder;
use ic_helpers::candid_header::{validate_header, CandidHeader, TypeCheckResult};
use ic_helpers::management;
use ic_storage::stable::Versioned;
use std::collections::HashMap;
use std::{cell::RefCell, rc::Rc};

pub trait FactoryCanister: Canister + Sized + PreUpdate {
    #[state_getter]
    fn factory_state(&self) -> Rc<RefCell<FactoryState>>;

    /// Returns the checksum of a wasm module in hex representation.
    #[query(trait = true)]
    fn get_checksum(&self) -> Result<String, FactoryError> {
        Ok(hex::encode(&self.factory_state().borrow().module()?.hash()))
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

    fn check_all_states<T: CandidType + Versioned>(
        &self,
    ) -> AsyncReturn<HashMap<Principal, TypeCheckResult>> {
        Box::pin(async move {
            let canisters = self.factory_state().borrow().canister_list();
            let mut results = HashMap::default();

            for canister in canisters {
                match virtual_canister_call!(canister, "state_check", (), CandidHeader).await {
                    Ok(canister_header) => {
                        results.insert(canister, validate_header::<T>(&canister_header))
                    }
                    Err(e) => results.insert(
                        canister,
                        TypeCheckResult::Error {
                            remote_version: 0,
                            current_version: T::version(),
                            error_message: format!(
                                "Failed to query canister state header: {}",
                                e.1
                            ),
                        },
                    ),
                };
            }

            results
        })
    }

    fn set_canister_code<T: CandidType + Versioned>(
        &self,
        wasm: Vec<u8>,
        state_header: CandidHeader,
    ) -> Result<u32, FactoryError> {
        let validate_res = validate_header::<T>(&state_header);
        if validate_res.is_err() {
            return Err(FactoryError::StateCheckFailed(HashMap::from([(
                self.principal(),
                validate_res,
            )])));
        }

        self
            .factory_state()
            .borrow_mut()
            .check_is_owner()?
            .set_canister_wasm(wasm, state_header)
    }

    #[allow(unused_variables)]
    fn create_canister<'a, T: ArgumentEncoder + 'a>(
        &'a self,
        init_args: T,
        controller: Option<Principal>,
        caller: Option<Principal>,
    ) -> AsyncReturn<'a, Result<Principal, FactoryError>> {
        Box::pin(async move {
            let state_lock = self.factory_state().borrow_mut().lock()?;

            let cycles = {
                #[cfg(target_arch = "wasm32")]
                {
                    let caller = caller.unwrap_or_else(ic_kit::ic::caller);

                    self.factory_state()
                        .borrow()
                        .consume_provided_cycles_or_icp(caller)
                        .await?
                }

                #[cfg(not(target_arch = "wasm32"))]
                {
                    0
                }
            };

            let principal = self
                .factory_state()
                .borrow()
                .create_canister(init_args, cycles, &state_lock, controller)?
                .await
                .map_err(|e| FactoryError::CanisterCreateFailed(e.1))?;

            self.factory_state()
                .borrow_mut()
                .register_created(principal, &state_lock)
                .expect("correct state lock");

            Ok(principal)
        })
    }

    fn upgrade_canister<T: CandidType + Versioned>(
        &mut self,
    ) -> AsyncReturn<Result<HashMap<Principal, UpgradeResult>, FactoryError>> {
        Box::pin(async move {
            let state_rc = self.factory_state();
            let state_lock = state_rc.borrow_mut().lock()?;

            let caller = ic_canister::ic_kit::ic::caller();

            let state_checks = self.check_all_states::<T>().await;
            if state_checks
                .iter()
                .any(|(_, res)| matches!(res, TypeCheckResult::Error { .. }))
            {
                return Err(FactoryError::StateCheckFailed(state_checks));
            }

            let module_hash = state_rc.borrow().module()?.hash().clone();

            let mut results = HashMap::new();
            for (canister, _) in state_checks {
                if state_rc.borrow().canisters()[&canister] == module_hash {
                    results.insert(canister, UpgradeResult::Noop);
                    continue;
                }

                let upgrader = state_rc
                    .borrow_mut()
                    .check_is_owner_internal(caller)?
                    .upgrade(canister, &state_lock)?;

                let upgrade_result = match upgrader.await {
                    Ok(()) => UpgradeResult::Upgraded,
                    Err(e) => UpgradeResult::Error(e.1),
                };

                results.insert(canister, upgrade_result);
            }

            {
                let mut state = state_rc.borrow_mut();
                let mut state = state.check_is_owner_internal(caller)?;
                for (canister, upgrade_result) in results.iter() {
                    if matches!(upgrade_result, UpgradeResult::Upgraded) {
                        state
                            .register_upgraded(*canister, &state_lock)
                            .expect("correct lock");
                    }
                }
            }

            Ok(results)
        })
    }

    #[update(trait = true)]
    fn reset_update_lock(&self) -> Result<(), FactoryError> {
        self.factory_state()
            .borrow_mut()
            .check_is_owner()?
            .release_update_lock();
        Ok(())
    }

    /// Returns the current version of canister.
    #[query(trait = true)]
    fn version(&self) -> Result<u32, FactoryError> {
        Ok(self.factory_state().borrow().module()?.version())
    }

    /// Returns the number of canisters created by the factory.
    #[query(trait = true)]
    fn length(&self) -> usize {
        self.factory_state().borrow().canister_count()
    }

    /// Returns a vector of all canisters created by the factory.
    #[query(trait = true)]
    fn get_all(&self) -> Vec<Principal> {
        self.factory_state().borrow().canister_list()
    }

    /// Returns the ICP fee amount for canister creation.
    #[query(trait = true)]
    fn get_icp_fee(&self) -> u64 {
        self.factory_state().borrow().icp_fee()
    }

    /// Sets the ICP fee amount for canister creation. This method can only be called
    /// by the factory controller.
    #[update(trait = true)]
    fn set_icp_fee(&self, e8s: u64) -> Result<(), FactoryError> {
        self.factory_state()
            .borrow_mut()
            .check_is_owner()?
            .set_icp_fee(e8s)
    }

    /// Returns the principal that will receive the ICP fees.
    #[query(trait = true)]
    fn get_icp_to(&self) -> Principal {
        self.factory_state().borrow().icp_to()
    }

    /// Sets the principal that will receive the ICP fees. This method can only be called
    /// by the factory controller.
    #[update(trait = true)]
    fn set_icp_to(&self, to: Principal) -> Result<(), FactoryError> {
        self.factory_state()
            .borrow_mut()
            .check_is_owner()?
            .set_fee_to(to)
    }

    /// Returns the ICPs transferred to the factory by the caller. This method returns all
    /// not used ICP minus transaction fee.
    #[update(trait = true)]
    fn refund_icp<'a>(&'a self) -> AsyncReturn<'a, Result<u64, FactoryError>> {
        use ic_helpers::ledger::{
            LedgerPrincipalExt, PrincipalId, Subaccount, DEFAULT_TRANSFER_FEE,
        };

        let ledger = self.factory_state().borrow().ledger_principal();
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
        self.factory_state()
            .borrow_mut()
            .check_is_owner()?
            .set_controller(controller)
    }

    /// Returns the factory controller principal.
    #[query(trait = true)]
    fn get_controller(&self) -> Principal {
        self.factory_state().borrow().controller()
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

    fn drop_canister(
        &self,
        canister_id: Principal,
        caller: Option<Principal>,
    ) -> AsyncReturn<Result<(), FactoryError>> {
        Box::pin(async move {
            let state_lock = self.factory_state().borrow_mut().lock()?;
            let caller = caller.unwrap_or_else(ic_kit::ic::caller);

            self.factory_state()
                .borrow_mut()
                .check_is_owner_internal(caller)?
                .drop_canister(canister_id, &state_lock)
                .await?;

            self.factory_state()
                .borrow_mut()
                .check_is_owner_internal(caller)?
                .register_dropped(canister_id, &state_lock)
        })
    }
}

#[derive(Debug, CandidType)]
pub enum UpgradeResult {
    Noop,
    Upgraded,
    Error(String),
}

generate_exports!(FactoryCanister);
