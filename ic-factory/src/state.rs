use crate::core::{create_canister, drop_canister, upgrade_canister};
use crate::error::FactoryError;
use crate::top_up;
use crate::update_lock::UpdateLock;
use ic_canister::virtual_canister_call;
use ic_cdk::api::call::CallResult;
use ic_cdk::export::candid::utils::ArgumentEncoder;
use ic_cdk::export::candid::{CandidType, Deserialize, Principal};
use ic_helpers::candid_header::CandidHeader;
use ic_helpers::ledger::{LedgerPrincipalExt, PrincipalId, DEFAULT_TRANSFER_FEE};
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;
use ledger_canister::{
    AccountIdentifier, BlockHeight, Subaccount, Tokens, TransferArgs, TransferError,
};
use std::collections::HashMap;
use std::future::Future;
use v1::{Factory, FactoryStateV1};

pub mod v1;

pub const DEFAULT_ICP_FEE: u64 = 10u64.pow(8);

type CanisterHash = Vec<u8>;

#[derive(Debug, Default, CandidType, Deserialize, IcStorage)]
pub struct FactoryState {
    /// Immutable configuration of the factory.
    pub configuration: FactoryConfiguration,
    /// Module that will be used for upgrading canisters on factory owns.
    upgrading_module: Option<CanisterModule>,
    /// Canisters that were created by the factory.
    canisters: HashMap<Principal, CanisterHash>,
    /// A flag used for locking the factory during the upgrade to prevent malforming the canister states.
    update_lock: UpdateLock,
}

#[derive(Debug, CandidType, Deserialize)]
pub struct CanisterModule {
    /// The canister wasm.
    wasm: Vec<u8>,
    /// Canister wasm hash.
    hash: CanisterHash,
    /// Canister state version.
    version: u32,
    /// Candid-serialized definition of the canister state type.
    state_header: CandidHeader,
}

impl CanisterModule {
    pub fn hash(&self) -> &CanisterHash {
        &self.hash
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}

impl Versioned for FactoryState {
    type Previous = FactoryStateV1;

    fn upgrade(prev: Self::Previous) -> Self {
        let FactoryStateV1 {
            configuration,
            factory,
        } = prev;
        let Factory {
            canisters,
            checksum,
        } = factory;

        let hash = checksum.hash;

        Self {
            configuration,

            // After the upgrade the canister wasm module would have to be uploaded again to
            // provide the state header.
            upgrading_module: None,

            // We assume for now that the canisters were not modified by external controllers, as
            // we didn't keep track of each canister has before.
            canisters: canisters
                .into_iter()
                .map(|(principal, _)| (principal, hash.clone()))
                .collect(),

            update_lock: UpdateLock::default(),
        }
    }
}

impl FactoryState {
    pub fn new(configuration: FactoryConfiguration) -> Self {
        Self {
            configuration,
            ..Default::default()
        }
    }

    /// Checks if the request caller is the factory conctroller (owner).
    ///
    /// # Errors
    ///
    /// Returns `FactoryError::AccessDenied` if the caller is not the factory controller.
    pub fn check_is_owner(&mut self) -> Result<Authorized<Owner>, FactoryError> {
        let caller = ic_canister::ic_kit::ic::caller();
        self.check_is_owner_internal(caller)
    }

    /// This is needed to deal with ic pecularity, where we cannot call `ic_cdk::caller()`
    /// twice in the same endpiont, hence we need to store it as a separate variable and
    /// pass it around.
    ///
    /// More on that:
    /// https://forum.dfinity.org/t/canister-violated-contract-ic0-msg-caller-size-cannot-be-executed-in-reply-callback-mode/7890/4
    pub fn check_is_owner_internal(
        &mut self,
        caller: Principal,
    ) -> Result<Authorized<Owner>, FactoryError> {
        if caller == self.configuration.controller {
            Ok(Authorized::<Owner<'_>> {
                auth: Owner { factory: self },
            })
        } else {
            Err(FactoryError::AccessDenied)
        }
    }

    /// Returns the controller (owner) of the factory.
    pub fn controller(&self) -> Principal {
        self.configuration.controller
    }

    /// Returns the ICP ledger principal that the factory work with.
    pub fn ledger_principal(&self) -> Principal {
        self.configuration.ledger_principal
    }

    /// Returns the icp_fee configuration.
    pub fn icp_fee(&self) -> u64 {
        self.configuration.icp_fee
    }

    /// Returns the icp_to configuration.
    pub fn icp_to(&self) -> Principal {
        self.configuration.icp_to
    }

    /// Creates a new canister with the wasm code stored in the factory state.
    ///
    /// Arguments:
    /// * `init_args` - arguments to send to the canister `init` method.
    /// * `cycles` - number of cycles to create the canister with. The factory must have enough
    ///   cycles as they are reduced from the factory balance.
    /// * `lock` - state update lock. This is used to ensure that the the factory state is editable
    ///   while the new canister is been created.
    /// * `controller` - additional controller principal to set for the created canister. The
    ///   factory always sets itself as a controller, but if this option is not `None`, the given
    ///   principal will be a second controller of the canister.
    ///
    /// This method returns a future that does not require the `FactoryState` to be borrowed when
    /// it is awaited. This design allows us drop the state borrow before making the async call to
    /// prevent possible `BorrowError`s and braking the state. This also means that this method
    /// cannot write the resulting canister principal into the list of the canisters, so the
    /// calling function must call [`register_created`] method after successful canister creation.
    ///
    /// To prevent incorrect usage of this method it is `pub(crate)`. Dependant crates should use
    /// [`FactoryCanister::create_canister`] method instead, that takes care of all this.
    ///
    /// # Errors
    ///
    /// Returns `FactoryError::CanisterWasmNotSet` if the canister code is not set.
    ///
    /// # Panics
    ///
    /// If the given lock is not the factory's lock. This should never happen if the factory code
    /// is written correctly.
    pub(crate) fn create_canister<A: ArgumentEncoder>(
        &self,
        init_args: A,
        cycles: u64,
        lock: &UpdateLock,
        controller: Option<Principal>,
    ) -> Result<impl Future<Output = CallResult<Principal>>, FactoryError> {
        self.check_lock(lock);

        let wasm = self.module()?.wasm.clone();
        Ok(create_canister(
            wasm,
            init_args,
            cycles,
            controller.map(|p| vec![ic_canister::ic_kit::ic::id(), p]),
        ))
    }

    /// Writes a new canister to the list of the factory canisters. It assumes that the canister
    /// was created with the wasm that is currently in the `FactoryState::module` field.
    ///
    /// # Errors
    ///
    /// Returns `FactoryError::CanisterWasmNotSet` if the canister code is not set.
    ///
    /// # Panics
    ///
    /// If the given lock is not the factory's lock. This should never happen if the factory code
    /// is written correctly.
    pub(crate) fn register_created(
        &mut self,
        canister_id: Principal,
        lock: &UpdateLock,
    ) -> Result<(), FactoryError> {
        self.check_lock(lock);
        self.canisters
            .insert(canister_id, self.module()?.hash.clone());
        Ok(())
    }

    /// Returns information about the wasm code the factory uses to create canisters.
    pub fn module(&self) -> Result<&CanisterModule, FactoryError> {
        self.upgrading_module
            .as_ref()
            .ok_or(FactoryError::CanisterWasmNotSet)
    }

    /// Number of canisters the factory keeps track of.
    pub fn canister_count(&self) -> usize {
        self.canisters.len()
    }

    /// List of canisters the factory keeps track of.
    pub fn canister_list(&self) -> Vec<Principal> {
        self.canisters.keys().copied().collect()
    }

    /// HashMap of canisters the factory keeps track of with their code hashes.
    pub fn canisters(&self) -> &HashMap<Principal, CanisterHash> {
        &self.canisters
    }

    /// Locks the `FactoryState`, prohibiting any update operations on it until the returned lock
    /// object is dropped. See [`UpdateLock`] documentation for more details about how and why this works.
    pub fn lock(&mut self) -> Result<UpdateLock, FactoryError> {
        self.update_lock.lock()
    }

    fn check_update_allowed(&self) -> Result<(), FactoryError> {
        match self.update_lock.is_locked() {
            true => Err(FactoryError::StateLocked),
            false => Ok(()),
        }
    }

    // If the caller can provide a lock to this method, it means that the state is locked, as there
    // is no way to get the lock object except by calling the [`lock`] method. So the purpose of
    // this check is to guard against creating a completely different `UpdateLock` object and
    // giving it to a factory method. In such case we simply panic to make it clear that the code
    // that did such a thing is broken and must be fixed.
    fn check_lock(&self, lock: &UpdateLock) {
        assert_eq!(*lock, self.update_lock, "invalid update lock usage")
    }

    /// Consumes the fee for canister creation in the form of cycles (if provided by the call) or
    /// ICP in other case. Returns an error in case nor cycles nor ICP are provided and the caller
    /// is not the factory controller.
    pub fn consume_provided_cycles_or_icp(
        &self,
        caller: Principal,
    ) -> impl Future<Output = Result<u64, FactoryError>> {
        let ledger = self.ledger_principal();
        let icp_to = self.icp_to();
        let icp_fee = self.icp_fee();
        let controller = self.controller();

        consume_provided_icp(caller, ledger, icp_to, icp_fee, controller)
    }
}

/// Abstraction to provided compile time checks for factory method access.
pub struct Authorized<T> {
    auth: T,
}

/// The operation caller is the factory controller (owner).
pub struct Owner<'a> {
    factory: &'a mut FactoryState,
}

impl<'a> Authorized<Owner<'a>> {
    /// Sets the new version of the wasm code that is used to create new canisters. The
    /// `state_header` argument must provide the current canister state descrition.
    pub fn set_canister_wasm(
        &mut self,
        wasm: Vec<u8>,
        state_header: CandidHeader,
    ) -> Result<u32, FactoryError> {
        self.auth.factory.check_update_allowed()?;
        let module_version = self
            .auth
            .factory
            .upgrading_module
            .as_ref()
            .map(|m| m.version)
            .unwrap_or(0);
        let hash = get_canister_hash(&wasm);

        let module = CanisterModule {
            wasm,
            hash,
            version: module_version,
            state_header,
        };

        self.auth.factory.upgrading_module = Some(module);
        Ok(module_version)
    }

    /// Update the factory controller.
    pub fn set_controller(&mut self, controller: Principal) -> Result<(), FactoryError> {
        self.auth.factory.check_update_allowed()?;
        self.auth.factory.configuration.controller = controller;

        Ok(())
    }

    /// Update the ledger principal.
    pub fn set_ledger_principal(
        &mut self,
        ledger_principal: Principal,
    ) -> Result<(), FactoryError> {
        self.auth.factory.check_update_allowed()?;
        self.auth.factory.configuration.ledger_principal = ledger_principal;

        Ok(())
    }

    /// Update the icp_fee configuration.
    pub fn set_icp_fee(&mut self, fee: u64) -> Result<(), FactoryError> {
        self.auth.factory.check_update_allowed()?;
        self.auth.factory.configuration.icp_fee = fee;

        Ok(())
    }

    /// Update the icp_to configuration.
    pub fn set_fee_to(&mut self, fee_to: Principal) -> Result<(), FactoryError> {
        self.auth.factory.check_update_allowed()?;
        self.auth.factory.configuration.icp_to = fee_to;

        Ok(())
    }

    /// Upgrade the code of the canister to the current module wasm code.
    ///
    /// This method works in a similar way to [`create_canister`], see its documentation for the
    /// details. [`register_upgraded`] method must be called after successfully awaiting on the
    /// returned future.
    pub(crate) fn upgrade(
        &self,
        canister_id: Principal,
        lock: &UpdateLock,
    ) -> Result<impl Future<Output = CallResult<()>>, FactoryError> {
        self.auth.factory.check_lock(lock);

        Ok(upgrade_canister(
            canister_id,
            self.auth.factory.module()?.wasm.clone(),
        ))
    }

    /// Updates the stored canister hash. Call this method after awaiting on [`upgrade`].
    pub(crate) fn register_upgraded(
        &mut self,
        canister_id: Principal,
        lock: &UpdateLock,
    ) -> Result<(), FactoryError> {
        self.auth.factory.check_lock(lock);
        self.auth
            .factory
            .canisters
            .insert(canister_id, self.auth.factory.module()?.hash.clone());

        Ok(())
    }

    /// Resets the factory state update lock to unlocked state. This method can be only called by
    /// the factory controller and is supposed to be used only in case the state was broken by some
    /// disaster.
    pub(crate) fn release_update_lock(&mut self) {
        self.auth.factory.update_lock.unlock()
    }

    /// Drops the canister.
    ///
    /// This method works in a similar way to [`create_canister`], see its documentation for the
    /// details. [`register_dropped`] must be called after awaiting on the reeturned fututre.
    pub(crate) fn drop_canister(
        &mut self,
        canister_id: Principal,
        lock: &UpdateLock,
    ) -> impl Future<Output = Result<(), FactoryError>> {
        self.auth.factory.check_lock(lock);
        drop_canister(canister_id)
    }

    /// Removes the canister from the list of tracked canisters.
    pub(crate) fn register_dropped(
        &mut self,
        canister_id: Principal,
        lock: &UpdateLock,
    ) -> Result<(), FactoryError> {
        self.auth.factory.check_lock(lock);
        match self.auth.factory.canisters.remove(&canister_id) {
            Some(_) => Ok(()),
            None => Err(FactoryError::NotFound),
        }
    }
}

fn get_canister_hash(wasm: &[u8]) -> CanisterHash {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(wasm);
    hasher.finalize().as_slice().into()
}

#[derive(Debug, CandidType, Deserialize)]
pub struct FactoryConfiguration {
    pub ledger_principal: Principal,
    pub icp_fee: u64,
    pub icp_to: Principal,
    pub controller: Principal,
}

impl FactoryConfiguration {
    pub fn new(
        ledger_principal: Principal,
        icp_fee: u64,
        icp_to: Principal,
        controller: Principal,
    ) -> Self {
        Self {
            ledger_principal,
            icp_fee,
            icp_to,
            controller,
        }
    }
}

impl Default for FactoryConfiguration {
    fn default() -> Self {
        Self {
            ledger_principal: Principal::anonymous(),
            icp_fee: DEFAULT_ICP_FEE,
            icp_to: Principal::anonymous(),
            controller: Principal::anonymous(),
        }
    }
}

// The canister creation fee is 10^12 cycles, so we require the provided amount to be a little larger for the operation for the cansiter.
// According to IC docs, 10^12 cycles should always cost 1 XDR, with is ~$1.
const MIN_CANISTER_CYCLES: u64 = 10u64.pow(12) * 5;

async fn consume_provided_icp(
    caller: Principal,
    ledger: Principal,
    icp_to: Principal,
    icp_fee: u64,
    controller: Principal,
) -> Result<u64, FactoryError> {
    if caller != controller {
        // If the caller is not the controller, we require the caller to provide cycles.
        return transfer_and_top_up(icp_fee, ledger, caller, icp_to).await;
    }

    Ok(MIN_CANISTER_CYCLES)
}

/// Transfers the ICP from the caller to the factory canister.
///  We transfer the minimum amount of ICP required to cover the canister creation fee,
/// and then we top up the Factory canister with the cycles.
/// We send the remaining ICP to the `icp_to` Principal.
async fn transfer_and_top_up(
    icp_fee: u64,
    ledger: Principal,
    caller: Principal,
    icp_to: Principal,
) -> Result<u64, FactoryError> {
    let id = ic_kit::ic::id();
    let balance = ledger
        .get_balance(id, Some((&PrincipalId(caller)).into()))
        .await
        .map_err(FactoryError::LedgerError)?;

    // defensive programming, maximum of twice the icp_fee
    let top_up_fee = top_up::cycles_to_icp(MIN_CANISTER_CYCLES)
        .await?
        .min(icp_fee);

    if balance - top_up_fee - icp_fee < 0 {
        Err(FactoryError::NotEnoughIcp(balance, top_up_fee + icp_fee))?;
    }

    let block_height =
        top_up::transfer_icp_to_cmc(top_up_fee, ledger, Subaccount::from(&PrincipalId(caller)))
            .await?;

    let cycles = top_up::mint_cycles_to_factory(block_height).await?;

    // Send the remaining ICP to the `icp_to` Principal
    send_remaining_fee_to(caller, icp_to, ledger, icp_fee).await?;

    Ok(cycles as u64)
}

/// Send the remainder fee to the `icp_to` Principal, after topping up the `Factory` canister with cycles
async fn send_remaining_fee_to(
    caller: Principal,
    icp_to: Principal,
    ledger: Principal,
    amount: u64,
) -> Result<(), FactoryError> {
    let args = TransferArgs {
        memo: Default::default(),
        amount: Tokens::from_e8s(amount - DEFAULT_TRANSFER_FEE.get_e8s()),
        fee: DEFAULT_TRANSFER_FEE,
        from_subaccount: Some(Subaccount::from(&PrincipalId(caller))),
        to: AccountIdentifier::new(PrincipalId(icp_to), None).to_address(),
        created_at_time: None,
    };

    virtual_canister_call!(ledger, "transfer", (args,), Result<BlockHeight, TransferError>)
        .await
        .map_err(|e| FactoryError::LedgerError(e.1))?
        .map_err(|e| FactoryError::LedgerError(format!("{e}")))?;

    Ok(())
}
