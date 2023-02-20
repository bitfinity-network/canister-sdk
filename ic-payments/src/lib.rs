use async_recursion::async_recursion;
use candid::{CandidType, Deserialize, Nat};
use error::PaymentError;
use ic_exports::ic_base_types::PrincipalId;
use ic_exports::ic_icrc1::{Account, Subaccount};
use ic_exports::ic_kit::ic;
use ic_exports::Principal;
use ic_helpers::tokens::Tokens128;
use icrc1::TokenTransferInfo;
use recovery_list::ForRecoveryList;

use crate::error::Result;

mod error;
mod icrc1;
mod recovery_list;

const N_RETRIES: usize = 3;
type Timestamp = u64;
type TxId = Nat;

#[derive(Debug, Eq, PartialEq, CandidType, Deserialize, Clone, Copy)]
pub enum Operation {
    None,
    CreditOnSuccess,
    CreditOnError,
}

#[derive(Debug, CandidType, Deserialize, Clone)]
pub struct Transfer {
    token: Principal,
    caller: Principal,
    from: Account,
    to: Account,
    amount: Tokens128,
    operation: Operation,
    r#type: TransferType,
    created_at: Timestamp,
}

const INTERMEDIATE_ACC_DOMAIN: &[u8] = b"is-amm-intermediate-acc";

impl Transfer {
    pub(crate) fn id(&self) -> [u8; 32] {
        use ic_exports::ic_crypto_sha::Sha224;

        let mut hash = Sha224::new();
        hash.write(INTERMEDIATE_ACC_DOMAIN);
        hash.write(self.caller.as_slice());
        hash.write(self.token.as_slice());

        hash.write(&self.created_at.to_le_bytes());

        let hash_result = hash.finish();
        let mut subaccount = [0; 32];
        subaccount[0..4].copy_from_slice(b"vfrc");
        subaccount[4..].copy_from_slice(&hash_result);

        subaccount
    }

    fn from(&self) -> Account {
        match self.r#type {
            TransferType::SingleStep => self.from.clone(),
            TransferType::DoubleStep(Stage::First) => self.from.clone(),
            TransferType::DoubleStep(Stage::Second) => self.interim_acc(),
        }
    }

    fn to(&self) -> Account {
        match self.r#type {
            TransferType::SingleStep => self.to.clone(),
            TransferType::DoubleStep(Stage::First) => self.interim_acc(),
            TransferType::DoubleStep(Stage::Second) => self.to.clone(),
        }
    }

    fn interim_acc(&self) -> Account {
        Account {
            owner: ic::id().into(),
            subaccount: Some(self.id()),
        }
    }
}

#[derive(Debug, CandidType, Deserialize, Clone, Copy)]
enum TransferType {
    SingleStep,
    DoubleStep(Stage),
}

#[derive(Debug, CandidType, Deserialize, Clone, Copy)]
enum Stage {
    First,
    Second,
}

pub trait Balances {
    fn credit(&mut self, recepient: Principal, amount: Tokens128) -> Result<Tokens128>;
}

pub struct Payment<T: Balances, const MEM_ID: u8> {
    token_principal: Principal,
    config: TokenConfiguration,
    balances: T,
}

impl<T: Balances + Sync + Send, const MEM_ID: u8> Payment<T, MEM_ID> {
    pub async fn deposit(&mut self, caller: Principal, amount: Tokens128) -> Result<TxId> {
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
            operation: Operation::CreditOnSuccess,
            r#type: TransferType::SingleStep,
            created_at: ic::time(),
        };

        self.transfer(transfer, N_RETRIES).await
    }

    pub async fn withdraw(&mut self, caller: Principal, amount: Tokens128) -> Result<TxId> {
        let this_id = ic::id();
        let from = PrincipalId(this_id).into();
        let to = PrincipalId(caller).into();

        let transfer = Transfer {
            token: self.token_principal,
            caller,
            from,
            to,
            amount,
            operation: Operation::CreditOnError,
            r#type: TransferType::DoubleStep(Stage::First),
            created_at: ic::time(),
        };

        self.transfer(transfer, N_RETRIES).await
    }

    #[async_recursion]
    pub async fn transfer(&mut self, transfer: Transfer, n_retries: usize) -> Result<TxId> {
        match icrc1::transfer_icrc1(
            self.token_principal,
            transfer.to(),
            transfer.amount,
            self.config.fee,
            transfer.from().subaccount,
        )
        .await
        {
            Ok(TokenTransferInfo { token_tx_id, .. }) => self.complete(transfer, token_tx_id).await,
            Err(PaymentError::Duplicate(tx_id)) => self.complete(transfer, tx_id).await,
            Err(PaymentError::MaybeFailed) => {
                self.retry(transfer, n_retries.saturating_sub(1)).await
            }
            Err(e) => self.reject(transfer, e),
        }
    }

    #[async_recursion]
    async fn complete(&mut self, transfer: Transfer, tx_id: TxId) -> Result<TxId> {
        match transfer.r#type {
            TransferType::DoubleStep(Stage::First) => {
                let next_step = Transfer {
                    r#type: TransferType::DoubleStep(Stage::Second),
                    ..transfer
                };
                self.transfer(next_step, N_RETRIES).await
            }
            _ => {
                if transfer.operation == Operation::CreditOnSuccess {
                    self.credit(transfer.caller, transfer.amount)?;
                }

                Ok(tx_id)
            }
        }
    }

    fn reject(&mut self, transfer: Transfer, error: PaymentError) -> Result<TxId> {
        match transfer.r#type {
            TransferType::DoubleStep(Stage::Second) => {
                // todo: handle bad fee case here
                self.add_for_recovery(transfer);
                Err(PaymentError::Recoverable)
            }
            _ => {
                if transfer.operation == Operation::CreditOnError {
                    self.credit(transfer.caller, transfer.amount)?;
                }

                Err(error)
            }
        }
    }

    async fn retry(&mut self, transfer: Transfer, n_retries: usize) -> Result<TxId> {
        if n_retries == 0 {
            self.add_for_recovery(transfer);
            return Err(PaymentError::Recoverable);
        }

        self.transfer(transfer, n_retries).await
    }

    pub fn set_fee(&mut self, fee: Tokens128) {
        self.config.fee = fee;
    }

    pub fn set_minting_principal(&mut self, minting_principal: Principal) {
        self.config.minting_principal = minting_principal;
    }

    fn credit(&mut self, recepient: Principal, amount: Tokens128) -> Result<Tokens128> {
        self.balances.credit(recepient, amount)
    }

    fn add_for_recovery(&self, transfer: Transfer) {
        ForRecoveryList::<MEM_ID>.push(transfer);
    }

    pub async fn recover_all(&mut self) -> Vec<Result<TxId>> {
        let mut results = vec![];
        while let Some(tx) = ForRecoveryList::<MEM_ID>.pop() {
            let result = self.transfer(tx, N_RETRIES).await;
            results.push(result);
        }

        results
    }
}

pub fn get_principal_subaccount(principal: Principal) -> Option<Subaccount> {
    Some(ic_exports::ledger::Subaccount::from(&PrincipalId(principal)).0)
}

#[derive(CandidType, Debug, Deserialize, Clone, Copy)]
pub struct TokenConfiguration {
    pub fee: Tokens128,
    pub minting_principal: Principal,
}
