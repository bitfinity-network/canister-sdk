pub mod state;
pub mod icrc1;
use ic_helpers::tokens::Tokens128;
pub use icrc1::{transfer, get_icrc1_balance};

use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use ic_canister::{generate_exports, query, state_getter, Canister, PreUpdate};
use ic_exports::{ic_cdk::export::candid::{CandidType, Deserialize}, ic_icrc1::Subaccount};
use ic_storage::IcStorage;
use std::collections::HashMap;
use is20_token_canister::canister::TokenCanister;
use is20_token::account::Account;
use futures::{future::BoxFuture, FutureExt};


pub enum Phase {
    PreFirstTransfer,
    MaybeFirstTransfer,
    PostFirstTransfer,
    MaybeSecondTransfer,
    PostSecondTransfer
}
pub struct ExtendedWithdrawalState {
    phase: Phase, 
    amount: Tokens128,
    token: Principal,
    holding_acc: Account,
    debit: Tokens128,
    receiver: Account
    
}

//TODO: specify minimum withdrawal fee to avoid griefing. 
impl ExtendedWithdrawalState {
    
    fn token_canister(&self) -> TokenCanister {
        TokenCanister::from_principal(self.token)
    }


    pub fn advance(&self, n_fails: u8) -> BoxFuture<'static, Result<(),()>> {
        
        async move {
        match self.phase {
            Phase::PreFirstTransfer => {
                transfer(self.token_canister(), self.holding_acc , self.debit, None).await;
                self.phase = Phase::MaybeFirstTransfer;
                self.advance(n_fails).await
            },
            Phase::MaybeFirstTransfer => {
                match get_icrc1_balance(self.token, self.holding_acc).await {
                   Ok(bal) if bal <= Tokens128::ZERO => self.phase = Phase::PreFirstTransfer,
                   Ok(bal)  => self.phase = Phase::PostFirstTransfer, 
                   _ => () 
                }
                self.advance(n_fails).await
            },
            Phase::PostFirstTransfer => {
                if let Ok(amount) = get_icrc1_balance(self.token, self.holding_acc).await {
                    transfer(self.token_canister(), self.receiver , amount, self.holding_acc.subaccount).await;
                    self.phase = Phase::MaybeSecondTransfer;
                }
                self.advance(n_fails).await
            },
            Phase::MaybeSecondTransfer => {
                match get_icrc1_balance(self.token, self.holding_acc).await {
                    Ok(bal) if bal > Tokens128::ZERO => self.phase = Phase::PostFirstTransfer,
                    Ok(bal)  => self.phase = Phase::PostSecondTransfer, 
                    _ => () 
                 }
                 self.advance(n_fails).await
            },
            Phase::PostSecondTransfer => Ok::<(),()>(())
            
            }
        }.boxed()
    }
}


type UserWithdrawals = HashMap<Principal, HashMap<u64, WithdrawalState>>;


pub trait Payments: Canister {
    #[state_getter]
    fn metrics(&self) -> Rc<RefCell<UserWithdrawals>>;

    #[query(trait = true)]
    fn advance(&self, ) -> MetricsData {
        curr_values()
    }

    #[query(trait = true)]
    fn get_metrics(&self) -> MetricsStorage {
        MetricsStorage::get().borrow().clone()
    }
}

fn curr_values() -> MetricsData {
    MetricsData {
        cycles: ic_exports::ic_kit::ic::balance(),
        stable_memory_size: {
            0
        },
        heap_memory_size: {
            0
        },
    }
}

#[derive(Debug, Copy, Clone, CandidType, Deserialize)]
pub enum Interval {
    PerMinute,
    PerHour,
    PerDay,
    PerWeek,
    Period { seconds: u64 },
}


generate_exports!(Payments);
