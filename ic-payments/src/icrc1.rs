use ic_canister::virtual_canister_call;
use ic_exports::candid::{CandidType, Nat, Principal};
use ic_exports::icrc_types::icrc1::account::{Account, Subaccount};
use ic_exports::icrc_types::icrc1::transfer::{Memo, TransferArg, TransferError};
use serde::Deserialize;

use crate::error::Result;
use crate::{Timestamp, TokenConfiguration, TxId};

#[derive(Debug, CandidType, Deserialize, Clone)]
pub struct TokenTransferInfo {
    /// Transaction id returned by the token canister.
    pub token_tx_id: TxId,
    /// Principal of the transferred token.
    pub token_principal: Principal,
    /// Amount of tokens were transferred to the principal.
    pub amount_transferred: Nat,
}

/// Returns current balance of the `account` in the ICRC-1 `token` canister.
pub async fn get_icrc1_balance(token: Principal, account: &Account) -> Result<Nat> {
    let res = virtual_canister_call!(token, "icrc1_balance_of", (account,), Nat).await?;

    Ok(res)
}

/// Requests a transfer in an ICRC-1 `token` canister.
pub async fn transfer_icrc1(
    token: Principal,
    to: Account,
    amount: Nat,
    fee: Nat,
    from_subaccount: Option<Subaccount>,
    created_at_time: Option<Timestamp>,
    memo: Option<Memo>,
) -> Result<TokenTransferInfo> {
    let args = TransferArg {
        from_subaccount,
        to,
        amount: amount.clone(),
        fee: Some(fee),
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

/// Requests fee and minting account configuration from an ICRC-1 canister.
pub async fn get_icrc1_configuration(token: Principal) -> Result<TokenConfiguration> {
    // ICRC-1 standard metadata doesn't include a minting account, so we have to do two requests
    // to get both fields, which is fine though since this is done once.
    let fee = get_icrc1_fee(token).await?;
    let minting_account = get_icrc1_minting_account(token).await?.unwrap_or(Account {
        owner: Principal::management_canister(),
        subaccount: None,
    });

    Ok(TokenConfiguration {
        principal: token,
        fee,
        minting_account,
    })
}

/// Requests fee configuration from an ICRC-1 canister.
pub async fn get_icrc1_fee(token: Principal) -> Result<Nat> {
    Ok(virtual_canister_call!(token, "icrc1_fee", (), Nat).await?)
}

/// Requests minting account configuration from an ICRC-1 canister.
pub async fn get_icrc1_minting_account(token: Principal) -> Result<Option<Account>> {
    Ok(virtual_canister_call!(token, "icrc1_minting_account", (), Option<Account>).await?)
}
