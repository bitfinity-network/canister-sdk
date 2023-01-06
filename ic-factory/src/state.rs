use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;

use candid::{Decode, Encode};
use ic_canister::virtual_canister_call;
use ic_exports::ic_base_types::PrincipalId;
use ic_exports::ic_cdk::api::call::CallResult;
use ic_exports::ic_cdk::export::candid::utils::ArgumentEncoder;
use ic_exports::ic_cdk::export::candid::{CandidType, Deserialize, Principal};
use ic_exports::ledger_canister::{
    AccountIdentifier, Subaccount, Tokens, TransferArgs, TransferError, DEFAULT_TRANSFER_FEE,
};
use ic_exports::{ic_kit, BlockHeight};
use ic_helpers::ledger::LedgerPrincipalExt;
use ic_stable_structures::{BoundedStorable, MemoryId, StableBTreeMap, StableCell, Storable};
use ic_storage::IcStorage;

use crate::core::{create_canister, drop_canister, upgrade_canister};
use crate::error::FactoryError;
use crate::top_up::{self, CYCLES_MINTING_CANISTER};
use crate::update_lock::UpdateLock;

pub mod v1;

pub const DEFAULT_ICP_FEE: u64 = 10u64.pow(8) * 2;

/// Amount of cycles to be charged by the factory when creating a new canister. This is needed to
/// cover the expenses made by the factory to create the canister (for all the update calls).
///
/// Actual amount needed to create a canister is around 1.5e9, so we use a slightly larger number
/// to be sure the factory doesn't run out of cycles.
pub const CANISTER_CREATION_CYCLE_COST: u64 = 10u64.pow(10);

/// Amount of cycles to transfer to the newly created canister. Actual amount available in the
/// canister will be slightly less due to canister creation fees.
pub const INITIAL_CANISTER_CYCLES: u64 = 5 * 10u64.pow(12);

#[derive(Debug, Deserialize, CandidType, Clone)]
pub struct CanisterHash(pub Vec<u8>);

impl From<&[u8]> for CanisterHash {
    fn from(hash: &[u8]) -> Self {
        // This will panic if we will change SHA-256 to an algorithm with other hash size.
        assert_eq!(hash.len(), Self::max_size() as usize);
        Self(hash.into())
    }
}

impl Storable for CanisterHash {
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        self.0.as_slice().into()
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl BoundedStorable for CanisterHash {
    fn max_size() -> u32 {
        // SHA-256 takes 32 bytes
        32
    }
}

#[derive(Debug, Default, CandidType, Deserialize, IcStorage)]
pub struct FactoryState {}

#[derive(Debug, CandidType, Deserialize, Clone)]
pub struct CanisterModule {
    /// The canister wasm.
    wasm: Vec<u8>,
    /// Canister wasm hash.
    hash: CanisterHash,
    /// Canister state version.
    version: u32,
}

#[derive(Debug, Default, IcStorage)]
pub struct CmcConfig {
    pub cmc_principal: Option<Principal>,
}

impl CmcConfig {
    pub fn cmc_principal(&self) -> Principal {
        self.cmc_principal.unwrap_or(CYCLES_MINTING_CANISTER)
    }
}

impl CanisterModule {
    pub fn hash(&self) -> &CanisterHash {
        &self.hash
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}

#[derive(Debug, CandidType, Deserialize)]
struct StorableCanisterModule(Option<CanisterModule>);

impl Storable for StorableCanisterModule {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Encode!(self)
            .expect("failed to serialize canister module")
            .into()
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        Decode!(&bytes, Self).expect("failed to deserialize canister module")
    }
}

impl FactoryState {
    pub fn reset(&mut self, configuration: FactoryConfiguration) {
        CONFIG_CELL.with(|cell| {
            cell.borrow_mut()
                .set(configuration)
                .expect("failed to set configuration to factory")
        });

        UPGRADING_MODULE_CELL.with(|cell| {
            cell.borrow_mut()
                .set(StorableCanisterModule(None))
                .expect("failed to set upgrading module to factory")
        });

        CANISTERS_MAP.with(|map| {
            map.borrow_mut().clear();
        });

        UPDATE_LOCK.with(|lock| lock.replace(UpdateLock::default()));
    }

    /// Checks if the request caller is the factory controller (owner).
    ///
    /// # Errors
    ///
    /// Returns `FactoryError::AccessDenied` if the caller is not the factory controller.
    pub fn check_is_owner(&mut self) -> Result<Authorized<Owner>, FactoryError> {
        let caller = ic_exports::ic_kit::ic::caller();
        self.check_is_owner_internal(caller)
    }

    /// This is needed to deal with ic peculiarity, where we cannot call `ic_cdk::caller()`
    /// twice in the same endpoint, hence we need to store it as a separate variable and
    /// pass it around.
    ///
    /// More on that:
    /// https://forum.dfinity.org/t/canister-violated-contract-ic0-msg-caller-size-cannot-be-executed-in-reply-callback-mode/7890/4
    pub fn check_is_owner_internal(
        &mut self,
        caller: Principal,
    ) -> Result<Authorized<Owner>, FactoryError> {
        if with_config(|cfg| cfg.controller == caller) {
            Ok(Authorized::<Owner> { _auth: Owner {} })
        } else {
            Err(FactoryError::AccessDenied)
        }
    }

    /// Returns the controller (owner) of the factory.
    pub fn controller(&self) -> Principal {
        with_config(|cfg| cfg.controller)
    }

    /// Sets the controller (owner) of the factory.
    fn set_controller(&mut self, controller: Principal) {
        update_config(|cfg| cfg.controller = controller);
    }

    /// Returns the ICP ledger principal that the factory work with.
    pub fn ledger_principal(&self) -> Principal {
        with_config(|cfg| cfg.ledger_principal)
    }

    /// Sets the ICP ledger principal.
    fn set_ledger_principal(&mut self, ledger: Principal) {
        update_config(|cfg| cfg.ledger_principal = ledger);
    }

    /// Returns the icp_fee configuration.
    pub fn icp_fee(&self) -> u64 {
        with_config(|cfg| cfg.icp_fee)
    }

    /// Sets the icp_fee configuration.
    fn set_icp_fee(&mut self, fee: u64) {
        update_config(|cfg| cfg.icp_fee = fee);
    }

    /// Returns the icp_to configuration.
    pub fn icp_to(&self) -> Principal {
        with_config(|cfg| cfg.icp_to)
    }

    /// Sets the icp_to configuration.
    fn set_icp_to(&mut self, to: Principal) {
        update_config(|cfg| cfg.icp_to = to);
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
    pub(crate) fn create_canister<A: ArgumentEncoder + Send>(
        &self,
        init_args: A,
        cycles: u64,
        lock: &UpdateLock,
        controller: Option<Principal>,
    ) -> Result<impl Future<Output = CallResult<Principal>>, FactoryError> {
        self.check_lock(lock);

        Ok(create_canister(
            self.module()?.wasm,
            init_args,
            cycles,
            controller.map(|p| vec![ic_exports::ic_kit::ic::id(), p]),
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

        let hash = self.module()?.hash;

        CANISTERS_MAP.with(|map| {
            map.borrow_mut()
                .insert(PrincipalKey(canister_id), hash)
                .expect("failed to insert canister hash to stable storage")
        });

        Ok(())
    }

    fn insert_canister(&mut self, canister_id: Principal, hash: CanisterHash) {
        CANISTERS_MAP.with(|map| {
            map.borrow_mut()
                .insert(PrincipalKey(canister_id), hash)
                .expect("failed to insert canister hash to stable storage")
        });
    }

    fn remove_canister(&mut self, canister_id: Principal) -> Option<CanisterHash> {
        CANISTERS_MAP.with(|map| map.borrow_mut().remove(&PrincipalKey(canister_id)))
    }

    /// Returns information about the wasm code the factory uses to create canisters.
    pub fn module(&self) -> Result<CanisterModule, FactoryError> {
        UPGRADING_MODULE_CELL
            .with(|cell| cell.borrow().get().0.clone())
            .ok_or(FactoryError::CanisterWasmNotSet)
    }

    /// Replaces canister module.
    fn set_upgrading_module(&mut self, new_module: Option<CanisterModule>) {
        UPGRADING_MODULE_CELL.with(|cell| {
            cell.borrow_mut()
                .set(StorableCanisterModule(new_module))
                .expect("failed to set upgrading canister module to stable storage")
        });
    }

    /// Number of canisters the factory keeps track of.
    pub fn canister_count(&self) -> usize {
        CANISTERS_MAP.with(|map| map.borrow().len()) as _
    }

    /// List of canisters the factory keeps track of.
    pub fn canister_list(&self) -> Vec<Principal> {
        CANISTERS_MAP.with(|map| map.borrow().iter().map(|(k, _)| k.0).collect())
    }

    /// HashMap of canisters the factory keeps track of with their code hashes.
    pub fn canisters(&self) -> HashMap<Principal, CanisterHash> {
        CANISTERS_MAP.with(|map| map.borrow().iter().map(|(k, v)| (k.0, v)).collect())
    }

    /// Locks the `FactoryState`, prohibiting any update operations on it until the returned lock
    /// object is dropped. See [`UpdateLock`] documentation for more details about how and why this works.
    pub fn lock(&mut self) -> Result<UpdateLock, FactoryError> {
        UPDATE_LOCK.with(|lock| lock.borrow_mut().lock())
    }

    /// Locks the `FactoryState`, prohibiting any update operations on it until the returned lock
    /// object is dropped. See [`UpdateLock`] documentation for more details about how and why this works.
    fn unlock(&mut self) {
        UPDATE_LOCK.with(|lock| lock.borrow_mut().unlock());
    }

    fn check_update_allowed(&self) -> Result<(), FactoryError> {
        match UPDATE_LOCK.with(|lock| lock.borrow().is_locked()) {
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
        UPDATE_LOCK.with(|inner_lock| {
            assert_eq!(*inner_lock.borrow(), *lock, "invalid update lock usage")
        });
    }

    /// Consumes the fee for canister creation in the form of cycles (if provided by the call) or
    /// ICP in other case. Returns an error in case nor cycles nor ICP are provided and the caller
    /// is not the factory controller.
    pub fn consume_provided_cycles_or_icp(
        &self,
        caller: Principal,
        cmc: Principal,
    ) -> impl Future<Output = Result<u64, FactoryError>> {
        let ledger = self.ledger_principal();
        let icp_to = self.icp_to();
        let icp_fee = self.icp_fee();
        let controller = self.controller();

        consume_provided_icp(caller, ledger, cmc, icp_to, icp_fee, controller)
    }

    /// Adds an existing canister to the canister list. This method does not have any information
    /// about the canister it is adding to the list, so it is responsibility of the caller to check
    /// if the canister exists and of correct type.
    pub fn register_existing(&mut self, canister_id: Principal) -> Result<(), FactoryError> {
        let _lock = self.lock()?;
        self.insert_canister(canister_id, self.module()?.hash);

        Ok(())
    }

    /// Removes the canister from the list of the factory canisters.
    pub fn forget(&mut self, canister_id: Principal) -> Result<(), FactoryError> {
        let _lock = self.lock()?;
        self.remove_canister(canister_id);

        Ok(())
    }
}

/// Abstraction to provided compile time checks for factory method access.
pub struct Authorized<T> {
    _auth: T,
}

/// The operation caller is the factory controller (owner).
pub struct Owner {}

impl Authorized<Owner> {
    /// Sets the new version of the wasm code that is used to create new canisters.
    pub fn set_canister_wasm(&mut self, wasm: Vec<u8>) -> Result<u32, FactoryError> {
        FactoryState::default().check_update_allowed()?;
        let module_version = FactoryState::default()
            .module()
            .map(|module| module.version)
            .unwrap_or(0);

        let hash = get_canister_hash(&wasm);

        let module = CanisterModule {
            wasm,
            hash,
            version: module_version,
        };

        factory_state().set_upgrading_module(Some(module));
        Ok(module_version)
    }

    /// Update the factory controller.
    pub fn set_controller(&mut self, controller: Principal) -> Result<(), FactoryError> {
        factory_state().check_update_allowed()?;
        factory_state().set_controller(controller);

        Ok(())
    }

    /// Update the ledger principal.
    pub fn set_ledger_principal(
        &mut self,
        ledger_principal: Principal,
    ) -> Result<(), FactoryError> {
        let mut state = factory_state();
        state.check_update_allowed()?;
        state.set_ledger_principal(ledger_principal);
        Ok(())
    }

    /// Update the icp_fee configuration.
    pub fn set_icp_fee(&mut self, fee: u64) -> Result<(), FactoryError> {
        let mut state = factory_state();
        state.check_update_allowed()?;
        state.set_icp_fee(fee);
        Ok(())
    }

    /// Update the icp_to configuration.
    pub fn set_fee_to(&mut self, fee_to: Principal) -> Result<(), FactoryError> {
        let mut state = factory_state();
        state.check_update_allowed()?;
        state.set_icp_to(fee_to);
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
        let state = factory_state();
        state.check_lock(lock);

        Ok(upgrade_canister(canister_id, state.module()?.wasm))
    }

    /// Updates the stored canister hash. Call this method after awaiting on [`upgrade`].
    pub(crate) fn register_upgraded(
        &mut self,
        canister_id: Principal,
        lock: &UpdateLock,
    ) -> Result<(), FactoryError> {
        let mut state = factory_state();
        state.check_lock(lock);
        let hash = state.module()?.hash;
        state.insert_canister(canister_id, hash);

        Ok(())
    }

    /// Resets the factory state update lock to unlocked state. This method can be only called by
    /// the factory controller and is supposed to be used only in case the state was broken by some
    /// disaster.
    pub(crate) fn release_update_lock(&mut self) {
        factory_state().unlock()
    }

    /// Drops the canister.
    ///
    /// This method works in a similar way to [`create_canister`], see its documentation for the
    /// details. [`register_dropped`] must be called after awaiting on the returned future.
    pub(crate) fn drop_canister(
        &mut self,
        canister_id: Principal,
        lock: &UpdateLock,
    ) -> impl Future<Output = Result<(), FactoryError>> {
        factory_state().check_lock(lock);
        drop_canister(canister_id)
    }

    /// Removes the canister from the list of tracked canisters.
    pub(crate) fn register_dropped(
        &mut self,
        canister_id: Principal,
        lock: &UpdateLock,
    ) -> Result<(), FactoryError> {
        let mut state = factory_state();
        state.check_lock(lock);
        match state.remove_canister(canister_id) {
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

#[derive(Debug, CandidType, Deserialize, Clone)]
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

impl Storable for FactoryConfiguration {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Encode!(self)
            .expect("failed to serialize factory configuration")
            .into()
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        Decode!(&bytes, Self).expect("failed to deserialize factory configuration")
    }
}

async fn consume_provided_icp(
    caller: Principal,
    ledger: Principal,
    cmc: Principal,
    icp_to: Principal,
    icp_fee: u64,
    controller: Principal,
) -> Result<u64, FactoryError> {
    if caller != controller {
        // If the caller is not the controller, we require the caller to provide cycles.
        return transfer_and_top_up(icp_fee, ledger, cmc, caller, icp_to).await;
    }

    Ok(CANISTER_CREATION_CYCLE_COST + INITIAL_CANISTER_CYCLES)
}

/// Converts the `INITIAL_CANISTER_CYCLES + CANISTER_CREATION_CYCLE_COST` to ICP tokens, and the caller sends
/// the tokens to the cycles-minting-canister, the factory canister
/// is topped up with cycles and the the icp_fee is sent to the
/// `icp_to` principal.
async fn transfer_and_top_up(
    icp_fee: u64,
    ledger: Principal,
    cmc: Principal,
    caller: Principal,
    icp_to: Principal,
) -> Result<u64, FactoryError> {
    let id = ic_kit::ic::id();
    let balance = ledger
        .get_balance(id, Some((&PrincipalId(caller)).into()))
        .await
        .map_err(FactoryError::LedgerError)?;

    if balance < icp_fee {
        Err(FactoryError::NotEnoughIcp(balance, icp_fee))?;
    }

    let top_up_fee =
        top_up::icp_amount_from_cycles(cmc, INITIAL_CANISTER_CYCLES + CANISTER_CREATION_CYCLE_COST)
            .await?;
    if top_up_fee > icp_fee {
        return Err(FactoryError::GenericError(format!(
            "The fee {} required to create {} cycles is greater than the ICP FEE {}",
            top_up_fee, INITIAL_CANISTER_CYCLES, icp_fee
        )))?;
    }

    let block_height = top_up::transfer_icp_to_cmc(
        cmc,
        top_up_fee,
        ledger,
        Subaccount::from(&PrincipalId(caller)),
    )
    .await?;

    let cycles = top_up::mint_cycles_to_factory(cmc, block_height).await?;

    // Send the remaining ICP to the `icp_to` Principal
    send_remaining_fee_to(caller, icp_to, ledger, icp_fee - top_up_fee).await?;

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

struct PrincipalKey(Principal);

impl Storable for PrincipalKey {
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        self.0.as_slice().into()
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        PrincipalKey(Principal::from_slice(&bytes))
    }
}

impl BoundedStorable for PrincipalKey {
    fn max_size() -> u32 {
        // max bytes count in Principal
        29
    }
}

const CONFIG_MEMORY_ID: MemoryId = MemoryId::new(0);
const UPGRADING_MODULE_MEMORY_ID: MemoryId = MemoryId::new(1);
const CANISTERS_MEMORY_ID: MemoryId = MemoryId::new(2);

thread_local! {
    static CONFIG_CELL: RefCell<StableCell<FactoryConfiguration>> = {
            RefCell::new(StableCell::new(CONFIG_MEMORY_ID, FactoryConfiguration::default())
                .expect("failed to initialize factory config"))
    };

    static UPGRADING_MODULE_CELL: RefCell<StableCell<StorableCanisterModule>> = {
        RefCell::new(StableCell::new(UPGRADING_MODULE_MEMORY_ID, StorableCanisterModule(None))
            .expect("failed to initialize factory upgrading module"))
    };

    static CANISTERS_MAP: RefCell<StableBTreeMap<PrincipalKey, CanisterHash>> =
        RefCell::new(StableBTreeMap::new(CANISTERS_MEMORY_ID));

    static UPDATE_LOCK: RefCell<UpdateLock> = RefCell::new(UpdateLock::default());
}

fn with_config<F, R>(f: F) -> R
where
    F: Fn(&FactoryConfiguration) -> R,
{
    CONFIG_CELL.with(|cell| f(cell.borrow().get()))
}

fn update_config<F>(f: F)
where
    F: Fn(&mut FactoryConfiguration),
{
    CONFIG_CELL.with(|cell| {
        let mut cell = cell.borrow_mut();
        let mut cfg = cell.get().clone();
        f(&mut cfg);
        cell.set(cfg)
            .expect("failed to set factory controller to stable memory")
    });
}

pub fn factory_state() -> FactoryState {
    FactoryState::default()
}
