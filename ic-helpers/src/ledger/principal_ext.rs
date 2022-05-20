use crate::ledger::{
    AccountIdentifier, BinaryAccountBalanceArgs, BlockHeight, Subaccount, Tokens, TransferArgs,
    TransferError, DEFAULT_TRANSFER_FEE,
};
use async_trait::async_trait;
use ic_base_types::PrincipalId;
use ic_cdk::export::candid::Principal;

#[async_trait]
pub trait LedgerPrincipalExt {
    async fn get_balance(
        &self,
        of: Principal,
        sub_account: Option<Subaccount>,
    ) -> Result<u64, String>;
    async fn transfer(
        &self,
        to: Principal,
        amount: u64,
        from_subaccount: Option<Subaccount>,
        to_subaccount: Option<Subaccount>,
    ) -> Result<u64, String>;
}

#[async_trait]
impl LedgerPrincipalExt for Principal {
    async fn get_balance(
        &self,
        of: Principal,
        sub_account: Option<Subaccount>,
    ) -> Result<u64, String> {
        let account = AccountIdentifier::new(of.into(), sub_account);
        let args = BinaryAccountBalanceArgs {
            account: account.to_address(),
        };
        let result = ic_cdk::call::<_, (Tokens,)>(*self, "account_balance", (args,))
            .await
            .map_err(|e| e.1)?
            .0;
        Ok(result.get_e8s())
    }

    async fn transfer(
        &self,
        to: Principal,
        amount: u64,
        from_subaccount: Option<Subaccount>,
        to_subaccount: Option<Subaccount>,
    ) -> Result<u64, String> {
        if amount < DEFAULT_TRANSFER_FEE.get_e8s() {
            return Err(format!(
                "cannot transfer tokens: amount '{}' is less then the fee '{}'",
                amount,
                DEFAULT_TRANSFER_FEE.get_e8s()
            ));
        }

        let args = TransferArgs {
            memo: Default::default(),
            amount: (Tokens::from_e8s(amount) - DEFAULT_TRANSFER_FEE)?,
            fee: DEFAULT_TRANSFER_FEE,
            from_subaccount,
            to: AccountIdentifier::new(PrincipalId(to), to_subaccount).to_address(),
            created_at_time: None,
        };

        ic_cdk::call::<_, (Result<BlockHeight, TransferError>,)>(*self, "transfer", (args,))
            .await
            .map_err(|e| e.1)?
            .0
            .map_err(|e| format!("{e:?}"))
    }
}
