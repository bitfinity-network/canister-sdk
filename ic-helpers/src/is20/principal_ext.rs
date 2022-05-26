use crate::management::{Canister, InstallCodeMode};
use async_trait::async_trait;
use candid::{encode_args, Nat, Principal};
use ic_cdk::api;
use ic_cdk::api::call::CallResult;
use ledger_canister::TransferError;
use std::convert::From;

#[async_trait]
pub trait IS20PrincipalExt {
    fn this() -> Self;
    fn check_access(target: Self);
    fn cycles() -> Nat;
    async fn balance_of(&self, address: Self) -> Nat;
    async fn transfer(&self, to: Self, amount: Nat) -> Result<Nat, String>;
    async fn transfer_include_fee(&self, to: Self, amount: Nat) -> Result<Nat, String>;
    async fn transfer_from(
        &self,
        from: Principal,
        to: Principal,
        amount: Nat,
    ) -> Result<Nat, String>;
    async fn mint(&self, to: Principal, amount: Nat) -> Nat;
    async fn burn(&self, to: Self, amount: Nat);
    async fn total_supply(&self) -> Nat;
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

    async fn balance_of(&self, address: Self) -> Nat {
        let canister_args = encode_args((address,)).unwrap_or_default();
        api::call::call::<_, (Nat,)>(*self, "balanceOf", (canister_args,))
            .await
            .map(|(amount,)| Nat::from(amount.0))
            .unwrap_or_default()
    }

    async fn transfer(&self, to: Self, amount: Nat) -> Result<Nat, String> {
        api::call::call::<_, (Result<Nat, TransferError>,)>(*self, "transfer", (to, amount))
            .await
            .map_err(|e| format!("{:?}", e))?
            .0
            .map_err(|e| format!("{:?}", e))
    }

    async fn transfer_include_fee(&self, to: Self, amount: Nat) -> Result<Nat, String> {
        api::call::call::<_, (Result<Nat, TransferError>,)>(
            *self,
            "transferIncludeFee",
            (to, amount),
        )
        .await
        .map_err(|e| format!("{:?}", e))?
        .0
        .map_err(|e| format!("{:?}", e))
    }

    async fn transfer_from(
        &self,
        from: Principal,
        to: Principal,
        amount: Nat,
    ) -> Result<Nat, String> {
        api::call::call::<_, (Result<Nat, TransferError>,)>(
            *self,
            "transferFrom",
            (from, to, amount),
        )
        .await
        .map_err(|e| format!("{:?}", e))?
        .0
        .map_err(|e| format!("{:?}", e))
    }

    async fn mint(&self, to: Principal, amount: Nat) -> Nat {
        let canister_args = encode_args((to, amount)).unwrap_or_default();
        api::call::call::<_, (Nat,)>(*self, "_mint", (canister_args,))
            .await
            .map(|(amount,)| amount)
            .unwrap_or_default()
    }

    async fn burn(&self, to: Self, amount: Nat) {
        let canister_args = encode_args((to, amount)).unwrap_or_default();
        let _ = api::call::call::<_, (Nat,)>(*self, "_burn", (canister_args,)).await;
    }

    async fn total_supply(&self) -> Nat {
        api::call::call::<_, (Nat,)>(*self, "totalSupply", ())
            .await
            .map(|(amount,)| amount)
            .unwrap_or_default()
    }

    async fn upgrade(&self, code: &[u8]) -> CallResult<()> {
        Canister::from(*self)
            .install_code(InstallCodeMode::Upgrade, code.to_vec(), ())
            .await
    }
}
