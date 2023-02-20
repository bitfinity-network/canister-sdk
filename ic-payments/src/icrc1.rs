use ic_canister::{canister_call, virtual_canister_call};
use ic_exports::candid::{CandidType, Nat};
use ic_exports::ic_icrc1::endpoints::{TransferArg, TransferError};
use ic_exports::ic_icrc1::{Account, Subaccount};
use ic_exports::serde::Deserialize;
use ic_exports::Principal;
use ic_helpers::tokens::Tokens128;

use crate::error::Result;

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

#[derive(CandidType, Debug, Deserialize, Clone, Copy)]
pub struct TokenConfiguration {
    pub fee: Tokens128,
    pub minting_principal: Principal,
}

pub async fn get_icrc1_balance(token: Principal, account: Account) -> Result<Tokens128> {
    Ok(virtual_canister_call!(token, "icrc1_balance_of", (account,), Tokens128).await?)
}

pub async fn transfer_icrc1(
    token: Principal,
    to: Account,
    amount: Tokens128,
    fee: Tokens128,
    from_subaccount: Option<Subaccount>,
) -> Result<TokenTransferInfo> {
    let args = TransferArg {
        from_subaccount,
        to,
        amount: amount.into(),
        fee: Some(fee.into()),
        memo: Default::default(),
        created_at_time: None,
    };

    let tx_id =
        virtual_canister_call!(token, "icrc1_transfer", (args,), std::result::Result<TxId, TransferError>)
            .await??;
    // .map_err(|(code, e)| match code {
    //     // With these error codes IC guarantees that the target canister state is not changed, so the transaction is
    //     // not applied
    //     RejectionCode::DestinationInvalid
    //     | RejectionCode::CanisterReject
    //     | RejectionCode::CanisterError => PairError::TokenTransferFailed(e.into()),
    //     // With other error codes we cannot be sure if the transaction is applied or not, so we must take special
    //     // care when dealing with these errors
    //     _ => PairError::TransactionMaybeFailed(token, e),
    // })?
    // .map_err(|e| PairError::TokenTransferFailed(e.into()))?;

    Ok(TokenTransferInfo {
        token_tx_id: tx_id.into(),
        amount_transferred: amount,
        token_principal: token,
    })
}

pub async fn get_icrc1_configuration(token: Principal) -> Option<TokenConfiguration> {
    // ICRC-1 standard metadata doesn't include minting account, so we have to do two requests
    // to get both fields. It's fine though since this is done only one time.
    let fee = get_icrc1_fee(token).await.ok()?;
    let minting_account = get_icrc1_minting_account(token).await.ok()?;

    let minting_principal = match minting_account {
        Some(v) if v.subaccount.is_none() => v.owner.0,
        _ => Principal::management_canister(),
    };

    Some(TokenConfiguration {
        fee,
        minting_principal,
    })
}

pub async fn get_icrc1_fee(token: Principal) -> Result<Tokens128> {
    Ok(virtual_canister_call!(token, "icrc1_fee", (), Tokens128).await?)
}

pub async fn get_icrc1_minting_account(token: Principal) -> Result<Option<Account>> {
    Ok(virtual_canister_call!(token, "icrc1_minting_account", (), Option<Account>).await?)
}
