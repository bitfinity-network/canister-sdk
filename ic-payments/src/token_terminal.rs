use async_recursion::async_recursion;
use candid::Principal;
use ic_exports::ic_base_types::PrincipalId;
use ic_exports::ic_icrc1::endpoints::TransferError;
use ic_exports::ic_icrc1::{Account, Subaccount};
use ic_exports::ic_kit::ic;
use ic_helpers::tokens::Tokens128;

use crate::error::{InternalPaymentError, PaymentError, RecoveryDetails, TransferFailReason};
use crate::icrc1::{self, get_icrc1_balance, TokenTransferInfo};
use crate::recovery_list::{RecoveryList, StableRecoveryList};
use crate::transfer::{Operation, Stage, Transfer, TransferType};
use crate::{Balances, TokenConfiguration, TxId};

/// Id that is used by the terminal to specify that the transaction ID is unknown, but it knows for
/// sure that the transaction exists.
pub const UNKNOWN_TX_ID: u128 = u64::MAX as u128;

/// Default number of retries in case of IC error, before a transfer stored into the list for
/// recovery.
const N_RETRIES: usize = 3;

/// Default period when deduplication of a transaction is possible. This is set by the token
/// implementation. 24 hours used here is the most common value, used by ICP and SNS-1 ledgers.
const DEFAULT_DEDUP_PERIOD: u64 = 10u64.pow(9) * 60 * 60 * 24;

/// Different IC nodes can have times not syncronized perfectly. We use 5 minute margin to make
/// sure we don't try to deduplicate transactions when it's not possible already.
const TX_WINDOW: u64 = 10u64.pow(9) * 60 * 5;

/// Bridge between an ICRC-1 token canister and the current canister. Provides safe and reliable
/// token transfer methods to and from the canister.
///
/// ```no_run
/// # use ic_exports::ic_kit::ic;
/// # use candid::Principal;
/// # use ic_helpers::tokens::Tokens128;
/// # use ic_payments::{TokenTerminal, BalanceError};
/// #
/// # struct BalancesImpl;
/// # impl ic_payments::Balances for BalancesImpl {
/// #     fn credit(
/// #         &mut self,
/// #         account_owner: Principal,
/// #         amount: Tokens128,
/// #     ) -> Result<Tokens128, BalanceError> { todo!() }
/// #     fn debit(
/// #         &mut self,
/// #         account_owner: Principal,
/// #         amount: Tokens128,
/// #     ) -> Result<Tokens128, BalanceError> { todo!() }
/// # }
/// # let token_principal = candid::Principal::management_canister();
/// # let balances_impl = BalancesImpl;
/// # let caller = ic::caller();
/// # let receiver = ic::caller();
/// # async {
///
/// // Configure the terminal
/// let token_config = ic_payments::icrc1::get_icrc1_configuration(token_principal).await?;
/// const STABLE_MEM_ID: u8 = 1;
/// let mut terminal = TokenTerminal::<_, STABLE_MEM_ID>::new(token_config.clone(), balances_impl);
///
/// // Recieve tokens from the `caller`. The received amount will be credited to the `caller` in
/// //`balances_impl`.
/// let (_tx_id, received) = terminal.deposit_all(caller).await?;
///
/// // Send tokens to the `caller`. The sent `received` amount will be deduced from the `caller`
/// // balance in `balances_impl`, but the actual amount the caller will receive to thier token
/// // account is `received - transfer_fee`.
/// let (_tx_id, sent) = terminal.withdraw(caller, received).await?;
///
/// assert_eq!(sent.amount, received.amount - token_config.fee.amount);
/// # Ok::<(), ic_payments::PaymentError>(())
/// # };
/// ```
///
/// # Generic parameters
/// * `B` - [`Balances`] storage.
/// * `R` - [`RecoveryList`] storage.
///
/// Note that for all types that implement either of the traits above, `Rc<RefCell<T>>` also
/// implement that trait. So to initiate an instance of `TokenTerminal` one can:
/// * use static implementations that can be cloned and given to the token terminal by value
/// * or give an `Rc<RefCell<T>>` of the value to the constructor.
#[derive(Debug)]
pub struct TokenTerminal<B: Balances, R: RecoveryList> {
    token_config: TokenConfiguration,
    balances: B,
    recovery_list: R,
    deduplication_period: u64,
}

impl<T: Balances + Sync + Send, const MEM_ID: u8> TokenTerminal<T, StableRecoveryList<MEM_ID>> {
    /// Creates a new terminal with the [default implementation of recovery
    /// list](StableRecoveryList).
    pub fn new(config: TokenConfiguration, balances: T) -> Self {
        let recovery_list = StableRecoveryList::<MEM_ID>;
        Self {
            token_config: config,
            balances,
            recovery_list,
            deduplication_period: DEFAULT_DEDUP_PERIOD,
        }
    }
}

impl<T: Balances + Sync + Send, R: RecoveryList> TokenTerminal<T, R> {
    /// Creates a new terminal.
    pub fn new_with_recovery_list(
        config: TokenConfiguration,
        balances: T,
        recovery_list: R,
    ) -> Self {
        Self {
            token_config: config,
            balances,
            recovery_list,
            deduplication_period: DEFAULT_DEDUP_PERIOD,
        }
    }
}

impl<T: Balances + Sync + Send, R: RecoveryList + Sync + Send> TokenTerminal<T, R> {
    /// Move all tokens from the depost interim account of the caller into caller's balance. See
    /// [`TokenTerminal::deposit`] for details.
    ///
    /// The amount the caller will recieve on their balance is `interim_account_balance -
    /// transfer_fee`, where `transfer_fee` is the fee set by the token canister.
    pub async fn deposit_all(
        &mut self,
        caller: Principal,
    ) -> Result<(TxId, Tokens128), PaymentError> {
        let account = get_deposit_interim_account(caller);
        let balance = get_icrc1_balance(self.token_config.principal, &account).await?;
        self.deposit(caller, balance).await
    }

    /// Move the specifed amount from the deposit interim account of the caller into caller's
    /// balance.
    ///
    /// This method implements default suggested flow for depositing tokens into the canister. The
    /// flow is:
    /// 1. Caller transfer tokens to the depoit interim acocunt.
    /// 2. Caller calls a method in the canister to initiate the deposit.
    /// 3. The canister transfers tokens to the its main account and credits the transferred amount
    ///    to the caller's balance.
    ///
    /// The amount that the caller will recieve on their balance is `interim_account_balance -
    /// transfer_fee` where `transfer_fee` is the fee set by the token canister.
    ///
    /// This method creates a single-step transfer from interim account to the main account of the
    /// `this` canister. This account id can be obtained by [`get_deposit_interim_account`] method
    /// (see the [`get_principal_subaccount`] docs for subaccount calculation algorithm).
    ///
    /// See the [crate level docs](index.html) for the details about single-step transfer
    /// recovery.
    pub async fn deposit(
        &mut self,
        caller: Principal,
        amount: Tokens128,
    ) -> Result<(TxId, Tokens128), PaymentError> {
        let to = PrincipalId(ic::id()).into();
        let transfer = Transfer::new(
            &self.token_config,
            caller,
            to,
            get_principal_subaccount(caller),
            amount,
        )
        .with_operation(Operation::CreditOnSuccess);
        let amount = transfer.final_amount()?;

        let tx_id = self.transfer(transfer, N_RETRIES).await?;

        Ok((tx_id, amount))
    }

    /// Move the specified amount from the caller's balance to the caller's main account.
    ///
    /// This method creates a double-step transfer using a subaccount unique for the transfer. The
    /// amount that the caller will recieve on their token account is `caller_balance -
    /// transfer_fee * 2` where `transfer_fee` is the fee set by the token canister.
    pub async fn withdraw(
        &mut self,
        caller: Principal,
        amount: Tokens128,
    ) -> Result<(TxId, Tokens128), PaymentError> {
        let to = PrincipalId(caller).into();

        let transfer = Transfer::new(&self.token_config, caller, to, None, amount)
            .double_step()
            .with_operation(Operation::CreditOnError);
        transfer.validate()?;
        let amount = transfer.final_amount()?;

        self.balances.debit(caller, transfer.amount())?;

        let tx_id = self.transfer(transfer, N_RETRIES).await?;

        Ok((tx_id, amount))
    }

    /// Executes the given [`transfer`](Transfer). If IC returns an error that does not guarantee
    /// either success or failure of the operation, the transaction will be retried `n_retries`
    /// times before saving it to the [recover_list`](RecoveryList).
    ///
    /// If the transaction succeeds or fails (e.g. it's not saved to the recovery list), the
    /// [transfer operation](Transfer.operation) is executed before the method returns.
    #[async_recursion]
    pub async fn transfer(
        &mut self,
        transfer: Transfer,
        n_retries: usize,
    ) -> Result<TxId, PaymentError> {
        transfer.validate()?;

        match transfer.execute().await {
            Ok(TokenTransferInfo { token_tx_id, .. }) => {
                Ok(self.complete(transfer, token_tx_id, n_retries).await?)
            }
            Err(InternalPaymentError::MaybeFailed) => {
                self.retry(transfer, n_retries.saturating_sub(1)).await
            }
            Err(e) => Ok(self.reject(transfer, e)?),
        }
    }

    #[async_recursion]
    async fn complete(
        &mut self,
        transfer: Transfer,
        tx_id: TxId,
        n_retries: usize,
    ) -> Result<TxId, PaymentError> {
        match transfer.next_step() {
            Some(t) => self.transfer(t, n_retries).await,
            None => {
                if transfer.operation() == Operation::CreditOnSuccess {
                    self.credit(transfer.caller(), transfer.amount_minus_fee())?;
                }

                Ok(tx_id)
            }
        }
    }

    fn reject(
        &mut self,
        transfer: Transfer,
        error: InternalPaymentError,
    ) -> Result<TxId, PaymentError> {
        match transfer.r#type() {
            TransferType::DoubleStep(Stage::Second, _) => {
                self.add_for_recovery(transfer);
                match error {
                    InternalPaymentError::WrongFee(fee) => {
                        Err(PaymentError::Recoverable(RecoveryDetails::BadFee(fee)))
                    }
                    _ => Err(PaymentError::Recoverable(RecoveryDetails::IcError)),
                }
            }
            _ => {
                if transfer.operation() == Operation::CreditOnError {
                    self.credit(transfer.caller(), transfer.amount())?;
                }

                Err(error.into())
            }
        }
    }

    async fn retry(&mut self, transfer: Transfer, n_retries: usize) -> Result<TxId, PaymentError> {
        if n_retries == 0 {
            self.add_for_recovery(transfer);
            return Err(PaymentError::Recoverable(RecoveryDetails::IcError));
        }

        match self.transfer(transfer.clone(), n_retries).await {
            Err(PaymentError::TransferFailed(TransferFailReason::Rejected(
                TransferError::Duplicate { duplicate_of },
            ))) => self.complete(transfer, duplicate_of, n_retries).await,
            result => result,
        }
    }

    /// Returns reference to balances structure used by the terminal.
    pub fn balances(&self) -> &T {
        &self.balances
    }

    /// Token configuration used by the terminal.
    pub fn token_config(&self) -> &TokenConfiguration {
        &self.token_config
    }

    /// Token transfer fee configured for the terminal.
    pub fn fee(&self) -> Tokens128 {
        self.token_config.fee
    }

    /// Token minting account configured for the terminal.
    pub fn minting_account(&self) -> &Account {
        &self.token_config.minting_account
    }

    /// Changes the token transfer fee configuration. This has effect on all new transfers as well
    /// as all transfers stored in the recovery list.
    pub fn set_fee(&mut self, fee: Tokens128) {
        self.token_config.fee = fee;
        self.update_recovery_fees();
    }

    /// Changes the minting account of the token. This has effect on all new transfers as well as
    /// all transfer stored in the recovery list.
    pub fn set_minting_account(&mut self, minting_account: Account) {
        self.token_config.minting_account = minting_account;
        self.update_recovery_fees();
    }

    fn update_recovery_fees(&mut self) {
        for tx in self.recovery_list.take_all() {
            let fee = self.token_config.get_fee(&tx.from_acc(), &tx.to);
            self.recovery_list.push(tx.with_fee(fee));
        }
    }

    fn credit(
        &mut self,
        recepient: Principal,
        amount: Tokens128,
    ) -> Result<Tokens128, PaymentError> {
        Ok(self.balances.credit(recepient, amount)?)
    }

    fn add_for_recovery(&mut self, transfer: Transfer) {
        self.recovery_list.push(transfer);
    }

    /// Recover all transfers stored in the recovery list. Exect stratagy of recovery depends for
    /// each transfer is decided by the transfer properties. Returns result of the recovery for
    /// each transfer in the recovery list. If the recovery list was empty, returns an empty list.
    ///
    /// After the transfer is recovered (by either successfully completing it, proving that it
    /// cannot be completed or proving that it was completed already), the transfer is removed from
    /// the list. If the recovery was not successful, e.g. if the terminal has still no proof
    /// weather the transfer is successful or not, the transfer is reterned to the recovery list.
    pub async fn recover_all(&mut self) -> Vec<Result<TxId, PaymentError>> {
        let mut results = vec![];
        for tx in self.recovery_list.take_all() {
            results.push(self.recover_tx(tx).await);
        }

        results
    }

    async fn recover_tx(&mut self, transfer: Transfer) -> Result<TxId, PaymentError> {
        if self.can_deduplicate(&transfer) {
            match self.transfer(transfer.clone(), N_RETRIES).await {
                Err(PaymentError::TransferFailed(TransferFailReason::Rejected(
                    TransferError::Duplicate { duplicate_of },
                ))) => self.complete(transfer, duplicate_of, N_RETRIES).await,
                result => result,
            }
        } else {
            self.recover_old_tx(transfer).await
        }
    }

    fn can_deduplicate(&self, tx: &Transfer) -> bool {
        ic::time().saturating_sub(tx.created_at()) < self.deduplication_period - TX_WINDOW
    }

    async fn recover_old_tx(&mut self, tx: Transfer) -> Result<TxId, PaymentError> {
        let TransferType::DoubleStep(stage, acc) = tx.r#type() else { return Err(PaymentError::TransferFailed(TransferFailReason::TooOld));};
        let interim_balance = icrc1::get_icrc1_balance(self.token_config.principal, acc).await?;

        match stage {
            Stage::First if interim_balance.is_zero() => self.reject(
                tx,
                InternalPaymentError::TransferFailed(TransferFailReason::Unknown),
            ),
            Stage::First => self.complete(tx, UNKNOWN_TX_ID.into(), N_RETRIES).await,
            Stage::Second if interim_balance.is_zero() => {
                self.complete(tx, UNKNOWN_TX_ID.into(), N_RETRIES).await
            }
            Stage::Second => Ok(self.transfer(tx.renew(), N_RETRIES).await?),
        }
    }

    /// Returns the list of transfers saved currectly in the recovery list. These transfers can be
    /// recovered by calling [`TokenTerminal::recover_all()`] method.
    pub fn list_for_recovery(&self) -> Vec<Transfer> {
        self.recovery_list.list()
    }
}

/// Returns the iterim account for deposit transfers. This account belongs to the `this` canister
/// and has subaccount derived from the `principal` (for details see [`get_principal_subaccount`]).
pub fn get_deposit_interim_account(principal: Principal) -> Account {
    Account {
        owner: ic::id().into(),
        subaccount: get_principal_subaccount(principal),
    }
}

/// Returns the subaccount id for the `pricnipal` for the deposit transfers. This subaccount is
/// calculated as:
/// Bytes[0..1] = principal.len()
/// Bytes[1..principal.len() + 1] = pricnipal.bytes()
/// Bytes[principal.len() + 1..32] = 0
pub fn get_principal_subaccount(principal: Principal) -> Option<Subaccount> {
    Some(ic_exports::ledger::Subaccount::from(&PrincipalId(principal)).0)
}
