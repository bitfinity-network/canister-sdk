use async_recursion::async_recursion;
use candid::Principal;
use ic_exports::ic_base_types::PrincipalId;
use ic_exports::ic_icrc1::{Account, Subaccount};
use ic_exports::ic_kit::ic;
use ic_helpers::tokens::Tokens128;

use crate::error::{InternalPaymentError, PaymentError, RecoveryDetails, TransferFailReason};
use crate::icrc1::{self, TokenTransferInfo};
use crate::recovery_list::ForRecoveryList;
use crate::transfer::{Operation, Stage, Transfer, TransferType};
use crate::{Balances, TokenConfiguration, TxId};

const N_RETRIES: usize = 3;

pub const UNKNOWN_TX_ID: u128 = u64::MAX as u128;

const DEFAULT_DEDUP_PERIOD: u64 = 10u64.pow(9) * 60 * 60 * 24;
const TX_WINDOW: u64 = 10u64.pow(9) * 60 * 5;

pub struct TokenTerminal<T: Balances, const MEM_ID: u8> {
    token_config: TokenConfiguration,
    balances: T,
    deduplication_period: u64,
}

impl<T: Balances + Sync + Send, const MEM_ID: u8> TokenTerminal<T, MEM_ID> {
    pub fn new(config: TokenConfiguration, balances: T) -> Self {
        Self {
            token_config: config,
            balances,
            deduplication_period: DEFAULT_DEDUP_PERIOD,
        }
    }

    pub async fn deposit(
        &mut self,
        caller: Principal,
        amount: Tokens128,
    ) -> Result<TxId, PaymentError> {
        let to = PrincipalId(caller).into();
        let transfer = Transfer::new(
            &self.token_config,
            caller,
            to,
            get_principal_subaccount(caller),
            amount,
        )
        .with_operation(Operation::CreditOnSuccess);

        self.transfer(transfer, N_RETRIES).await
    }

    pub async fn withdraw(
        &mut self,
        caller: Principal,
        amount: Tokens128,
    ) -> Result<TxId, PaymentError> {
        let to = PrincipalId(caller).into();

        let transfer = Transfer::new(&self.token_config, caller, to, None, amount)
            .double_step()
            .with_operation(Operation::CreditOnError);

        self.transfer(transfer, N_RETRIES).await
    }

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
            Err(InternalPaymentError::Duplicate(tx_id)) => {
                Ok(self.complete(transfer, tx_id, n_retries).await?)
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

                println!("transfer ok");
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
            println!("transfer err");
            self.add_for_recovery(transfer);
            return Err(PaymentError::Recoverable(RecoveryDetails::IcError));
        }

        self.transfer(transfer, n_retries).await
    }

    pub fn fee(&self) -> Tokens128 {
        self.token_config.fee
    }

    pub fn minting_account(&self) -> &Account {
        &self.token_config.minting_account
    }

    pub fn set_fee(&mut self, fee: Tokens128) {
        self.token_config.fee = fee;
    }

    pub fn set_minting_account(&mut self, mintint_account: Account) {
        self.token_config.minting_account = mintint_account;
    }

    fn credit(
        &mut self,
        recepient: Principal,
        amount: Tokens128,
    ) -> Result<Tokens128, InternalPaymentError> {
        self.balances.credit(recepient, amount)
    }

    fn add_for_recovery(&self, transfer: Transfer) {
        ForRecoveryList::<MEM_ID>.push(transfer);
    }

    pub async fn recover_all(&mut self) -> Vec<Result<TxId, PaymentError>> {
        let mut results = vec![];
        for tx in ForRecoveryList::<MEM_ID>.take_all() {
            results.push(self.recover_tx(tx).await);
        }

        results
    }

    async fn recover_tx(&mut self, tx: Transfer) -> Result<TxId, PaymentError> {
        if self.can_deduplicate(&tx) {
            self.transfer(tx, N_RETRIES).await
        } else {
            self.recover_old_tx(tx).await
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

    pub fn list_for_recovery(&self) -> Vec<Transfer> {
        ForRecoveryList::<MEM_ID>.list()
    }
}

pub fn get_principal_subaccount(principal: Principal) -> Option<Subaccount> {
    Some(ic_exports::ledger::Subaccount::from(&PrincipalId(principal)).0)
}
