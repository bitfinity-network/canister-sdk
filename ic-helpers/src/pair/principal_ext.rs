use ic_cdk::export::candid::Principal;
use async_trait::async_trait;

#[async_trait]
pub trait PairPrincipalExt {
    /// Sets the liquidity cap for the pair. This method can only be called by the pair owner,
    /// which is usually the pair factory.
    async fn set_cap(&self, amount: Option<u128>) -> Result<(), String>;
}

#[async_trait]
impl PairPrincipalExt for Principal {
    async fn set_cap(&self, amount: Option<u128>) -> Result<(), String>{
        ic_cdk::api::call::call::<_, ()>(*self, "set_cap", (amount,))
            .await
            .map_err(|e| format!("{:?}", e))
    }
}