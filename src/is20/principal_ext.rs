use crate::management::{Canister, InstallCodeMode};
use async_trait::async_trait;
use candid::{decode_args, encode_args, CandidType, Nat, Principal};
use ic_cdk::api;
use ic_cdk::api::call::CallResult;
use num_traits::cast::ToPrimitive;
use serde::Deserialize;
use std::convert::From;

#[derive(CandidType, Deserialize, Debug)]
pub struct Amount(u128);

impl From<Amount> for u128 {
    fn from(src: Amount) -> Self {
        src.0
    }
}

#[async_trait]
pub trait IS20PrincipalExt{
    fn this() -> Self;
    fn check_access(target: Self);
    fn cycles() -> Nat;
    async fn balance_of(&self, address: Self) -> u128;
    async fn transfer(&self, to: Self, amount: u128) -> Result<(), String>;
    async fn transfer_from(
        &self,
        from: Principal,
        to: Principal,
        amount: u128,
    ) -> Result<u128, String>;
    async fn mint(&self, to: Principal, amount: u128) -> u128;
    async fn burn(&self, to: Self, amount: u128);
    async fn total_supply(&self) -> u128;
    async fn upgrade(&self, code: &[u8]) -> CallResult<()>;
}

#[async_trait]
impl IS20PrincipalExt for Principal {
    fn this() -> Self {
        api::id()
    }

    fn check_access(target: Self) {
        if target != Principal::anonymous() && api::caller() != target {
            ic_cdk::trap("unauthorized access");
        }
    }

    fn cycles() -> Nat {
        api::canister_balance().into()
    }

    async fn balance_of(&self, address: Self) -> u128 {
        let canister_args = encode_args((address,)).unwrap_or_default();
        api::call::call::<_, (Amount,)>(*self, "balanceOf", (canister_args,))
            .await
            .map(|(amount,)| amount.into())
            .unwrap_or_default()
    }

    async fn transfer(&self, to: Self, amount: u128) -> Result<(), String> {
        #[derive(Deserialize, CandidType, Clone)]
        struct TransferArguments {
            pub to: Principal,
            pub amount: u128,
        }

        #[derive(CandidType, Debug, Deserialize)]
        enum TransferError {
            InsufficientBalance,
            AmountTooLarge,
            CallFailed,
            Unknown,
        }

        let args =
            encode_args((TransferArguments { to, amount },)).map_err(|e| format!("{:?}", e))?;
        let result = api::call::call_raw(*self, "transfer", args, 0)
            .await
            .map_err(|e| format!("{:?}", e))?;

        decode_args(&result).map_err(|e| format!("{}", e))
    }

    async fn transfer_from(
        &self,
        from: Principal,
        to: Principal,
        amount: u128,
    ) -> Result<u128, String> {
        #[derive(CandidType, Debug, Eq, PartialEq, Deserialize)]
        enum TxError {
            InsufficientAllowance,
            InsufficientBalance,
        }

        api::call::call::<_, (Result<Nat, TxError>,)>(*self, "transferFrom", (from, to, amount))
            .await
            .map_err(|e| format!("{:?}", e))?
            .0
            .map(|v| v.0.to_u128().unwrap())
            .map_err(|e| format!("{:?}", e))
    }

    async fn mint(&self, to: Principal, amount: u128) -> u128 {
        let canister_args = encode_args((to, amount)).unwrap_or_default();
        api::call::call::<_, (Amount,)>(*self, "_mint", (canister_args,))
            .await
            .map(|(amount,)| amount.into())
            .unwrap_or_default()
    }

    async fn burn(&self, to: Self, amount: u128) {
        let canister_args = encode_args((to, amount)).unwrap_or_default();
        let _ = api::call::call::<_, (Amount,)>(*self, "_burn", (canister_args,)).await;
    }

    async fn total_supply(&self) -> u128 {
        api::call::call::<_, (Amount,)>(*self, "totalSupply", ())
            .await
            .map(|(amount,)| amount.into())
            .unwrap_or_default()
    }

    async fn upgrade(&self, code: &[u8]) -> CallResult<()> {
        Canister::from(*self)
            .install_code(InstallCodeMode::Upgrade, code.to_vec(), ())
            .await
    }
}
