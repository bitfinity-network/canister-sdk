use async_trait::async_trait;
use ic_canister::virtual_canister_call;
use ic_exports::{
    ic_base_types::PrincipalId,
    ic_cdk::export::candid::Principal,
    ledger_canister::{
        AccountIdentifier, BinaryAccountBalanceArgs, Subaccount, Tokens, TransferArgs,
        TransferError, DEFAULT_TRANSFER_FEE,
    },
    BlockHeight,
};

use super::private::Sealed;

#[async_trait]
pub trait LedgerPrincipalExt: Sealed {
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
        virtual_canister_call!(*self, "account_balance", (args,), Tokens)
            .await
            .map(|tokens| tokens.get_e8s())
            .map_err(|e| e.1)
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

        virtual_canister_call!(*self, "transfer", (args,), Result<BlockHeight, TransferError>)
            .await
            .map_err(|e| e.1)?
            .map_err(|e| format!("{e:?}"))
    }
}
