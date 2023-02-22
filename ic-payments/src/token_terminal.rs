use async_recursion::async_recursion;
use candid::Principal;
use ic_exports::ic_base_types::PrincipalId;
use ic_exports::ic_icrc1::{Account, Subaccount};
use ic_exports::ic_kit::ic;
use ic_helpers::tokens::Tokens128;

use crate::error::{InternalPaymentError, PaymentError};
use crate::icrc1::{self, TokenTransferInfo};
use crate::recovery_list::ForRecoveryList;
use crate::transfer::{Operation, Stage, Transfer, TransferType};
use crate::{Balances, TokenConfiguration, TxId};

const N_RETRIES: usize = 3;

pub struct TokenTerminal<T: Balances, const MEM_ID: u8> {
    token_principal: Principal,
    config: TokenConfiguration,
    balances: T,
}

impl<T: Balances + Sync + Send, const MEM_ID: u8> TokenTerminal<T, MEM_ID> {
    pub fn new(token_principal: Principal, config: TokenConfiguration, balances: T) -> Self {
        Self {
            token_principal,
            config,
            balances,
        }
    }

    pub async fn deposit(
        &mut self,
        caller: Principal,
        amount: Tokens128,
    ) -> Result<TxId, PaymentError> {
        let this_id = ic::id();
        let from = Account {
            owner: this_id.into(),
            subaccount: get_principal_subaccount(caller),
        };

        let to = PrincipalId(caller).into();
        let transfer = Transfer {
            token: self.token_principal,
            caller,
            from,
            to,
            amount,
            fee: self.config.fee,
            operation: Operation::CreditOnSuccess,
            r#type: TransferType::SingleStep,
            created_at: ic::time(),
        };

        self.transfer(transfer, N_RETRIES).await
    }

    pub async fn withdraw(
        &mut self,
        caller: Principal,
        amount: Tokens128,
    ) -> Result<TxId, PaymentError> {
        let this_id = ic::id();
        let from = PrincipalId(this_id).into();
        let to = PrincipalId(caller).into();

        let transfer = Transfer {
            token: self.token_principal,
            caller,
            from,
            to,
            amount,
            fee: self.config.fee,
            operation: Operation::CreditOnError,
            r#type: TransferType::DoubleStep(Stage::First),
            created_at: ic::time(),
        };

        self.transfer(transfer, N_RETRIES).await
    }

    #[async_recursion]
    pub async fn transfer(
        &mut self,
        transfer: Transfer,
        n_retries: usize,
    ) -> Result<TxId, PaymentError> {
        transfer.validate()?;

        match icrc1::transfer_icrc1(
            self.token_principal,
            transfer.to(),
            transfer.amount(),
            self.config.get_fee(&transfer),
            transfer.from().subaccount,
            Some(transfer.created_at),
        )
        .await
        {
            Ok(TokenTransferInfo { token_tx_id, .. }) => {
                Ok(self.complete(transfer, token_tx_id).await?)
            }
            Err(InternalPaymentError::Duplicate(tx_id)) => {
                Ok(self.complete(transfer, tx_id).await?)
            }
            Err(InternalPaymentError::Recoverable) => {
                self.retry(transfer, n_retries.saturating_sub(1)).await
            }
            Err(e) => Ok(self.reject(transfer, e)?),
        }
    }

    #[async_recursion]
    async fn complete(&mut self, transfer: Transfer, tx_id: TxId) -> Result<TxId, PaymentError> {
        match transfer.r#type {
            TransferType::DoubleStep(Stage::First) => {
                let next_step = Transfer {
                    r#type: TransferType::DoubleStep(Stage::Second),
                    ..transfer
                };
                self.transfer(next_step, N_RETRIES).await
            }
            _ => {
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
    ) -> Result<TxId, InternalPaymentError> {
        match transfer.r#type {
            TransferType::DoubleStep(Stage::Second) => {
                // TODO: handle bad fee case here
                self.add_for_recovery(transfer);
                Err(InternalPaymentError::Recoverable)
            }
            _ => {
                if transfer.operation() == Operation::CreditOnError {
                    self.credit(transfer.caller(), transfer.amount())?;
                }

                Err(error)
            }
        }
    }

    async fn retry(&mut self, transfer: Transfer, n_retries: usize) -> Result<TxId, PaymentError> {
        if n_retries == 0 {
            self.add_for_recovery(transfer);
            return Err(PaymentError::Recoverable);
        }

        self.transfer(transfer, n_retries).await
    }

    pub fn fee(&self) -> Tokens128 {
        self.config.fee
    }

    pub fn minting_account(&self) -> &Account {
        &self.config.minting_account
    }

    pub fn set_fee(&mut self, fee: Tokens128) {
        self.config.fee = fee;
    }

    pub fn set_minting_account(&mut self, mintint_account: Account) {
        self.config.minting_account = mintint_account;
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
            let result = self.transfer(tx, N_RETRIES).await;
            results.push(result);
        }

        results
    }

    pub fn list_for_recovery(&self) -> Vec<Transfer> {
        ForRecoveryList::<MEM_ID>.list()
    }
}

pub fn get_principal_subaccount(principal: Principal) -> Option<Subaccount> {
    Some(ic_exports::ledger::Subaccount::from(&PrincipalId(principal)).0)
}
