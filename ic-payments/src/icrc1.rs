use ic_canister::virtual_canister_call;
use ic_exports::candid::{CandidType, Nat};
use ic_exports::ic_icrc1::endpoints::{TransferArg, TransferError};
use ic_exports::ic_icrc1::{Account, Memo, Subaccount};
use ic_exports::serde::Deserialize;
use ic_exports::Principal;
use ic_helpers::tokens::Tokens128;

use crate::error::{InternalPaymentError, Result};
use crate::{Timestamp, TokenConfiguration};

type TxId = Nat;

#[derive(Debug, CandidType, Deserialize, Clone)]
pub struct TokenTransferInfo {
    /// Transaction id returned by the token canister.
    pub token_tx_id: TxId,
    /// Principal of the transferred token.
    pub token_principal: Principal,
    /// Amount of tokens were transferred to the principal.
    pub amount_transferred: Tokens128,
}

pub async fn get_icrc1_balance(token: Principal, account: &Account) -> Result<Tokens128> {
    let result = virtual_canister_call!(token, "icrc1_balance_of", (account,), Nat).await?;
    Tokens128::from_nat(&result).ok_or(InternalPaymentError::Overflow)
}

pub async fn transfer_icrc1(
    token: Principal,
    to: Account,
    amount: Tokens128,
    fee: Tokens128,
    from_subaccount: Option<Subaccount>,
    created_at_time: Option<Timestamp>,
    memo: Option<Memo>,
) -> Result<TokenTransferInfo> {
    let args = TransferArg {
        from_subaccount,
        to,
        amount: amount.to_nat(),
        fee: Some(fee.to_nat()),
        memo,
        created_at_time,
    };

    let tx_id =
        virtual_canister_call!(token, "icrc1_transfer", (args,), std::result::Result<TxId, TransferError>)
            .await??;

    Ok(TokenTransferInfo {
        token_tx_id: tx_id,
        amount_transferred: amount,
        token_principal: token,
    })
}

pub async fn get_icrc1_configuration(token: Principal) -> Result<TokenConfiguration> {
    // ICRC-1 standard metadata doesn't include minting account, so we have to do two requests
    // to get both fields. It's fine though since this is done only one time.
    let fee = get_icrc1_fee(token).await?;
    let minting_account = get_icrc1_minting_account(token).await?.unwrap_or(Account {
        owner: Principal::management_canister().into(),
        subaccount: None,
    });

    Ok(TokenConfiguration {
        principal: token,
        fee,
        minting_account,
    })
}

pub async fn get_icrc1_fee(token: Principal) -> Result<Tokens128> {
    Ok(virtual_canister_call!(token, "icrc1_fee", (), Tokens128).await?)
}

pub async fn get_icrc1_minting_account(token: Principal) -> Result<Option<Account>> {
    Ok(virtual_canister_call!(token, "icrc1_minting_account", (), Option<Account>).await?)
}
