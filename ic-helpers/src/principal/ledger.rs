use std::future::Future;

use candid::CandidType;
use ic_canister::virtual_canister_call;
use ic_exports::candid::Principal;
use ic_exports::ic_cdk::call::CallResult;
use ic_exports::ledger::{
    AccountIdentifier, Memo, Subaccount, Tokens, TransferArgs, TransferError, DEFAULT_SUBACCOUNT,
};
use ic_exports::BlockHeight;
use serde::{Deserialize, Serialize};

use super::private::Sealed;

pub const DEFAULT_TRANSFER_FEE: Tokens = Tokens::from_e8s(10_000);

pub trait LedgerPrincipalExt: Sealed {
    fn get_balance(
        &self,
        of: Principal,
        sub_account: Option<Subaccount>,
    ) -> impl Future<Output = CallResult<u64>> + Send;

    fn transfer(
        &self,
        to: Principal,
        amount: u64,
        from_subaccount: Option<Subaccount>,
        to_subaccount: Option<Subaccount>,
    ) -> impl Future<Output = Result<u64, String>> + Send;
}

/// Arguments taken by the account_balance candid endpoint.
#[derive(Serialize, Deserialize, CandidType, Clone, Hash, Debug, PartialEq, Eq)]
pub struct BinaryAccountBalanceArgs {
    pub account: AccountIdentifier,
}

impl LedgerPrincipalExt for Principal {
    async fn get_balance(&self, of: Principal, sub_account: Option<Subaccount>) -> CallResult<u64> {
        let account =
            AccountIdentifier::new(&of, sub_account.as_ref().unwrap_or(&DEFAULT_SUBACCOUNT));
        let args = BinaryAccountBalanceArgs { account };
        virtual_canister_call!(*self, "account_balance", (args,), Tokens)
            .await
            .map(|tokens| tokens.e8s())
    }

    async fn transfer(
        &self,
        to: Principal,
        amount: u64,
        from_subaccount: Option<Subaccount>,
        to_subaccount: Option<Subaccount>,
    ) -> Result<u64, String> {
        if amount < DEFAULT_TRANSFER_FEE.e8s() {
            return Err(format!(
                "cannot transfer tokens: amount '{}' is less then the fee '{}'",
                amount,
                DEFAULT_TRANSFER_FEE.e8s()
            ));
        }

        let args = TransferArgs {
            memo: Memo(0),
            amount: Tokens::from_e8s(amount - DEFAULT_TRANSFER_FEE.e8s()),
            fee: DEFAULT_TRANSFER_FEE,
            from_subaccount,
            to: AccountIdentifier::new(&to, to_subaccount.as_ref().unwrap_or(&DEFAULT_SUBACCOUNT)),
            created_at_time: None,
        };

        virtual_canister_call!(*self, "transfer", (args,), Result<BlockHeight, TransferError>)
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("{e:?}"))
    }
}
