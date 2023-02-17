#[allow(unused_imports)]
use candid::{Nat, Principal};
use ic_canister::{canister_call, Canister};
use ic_helpers::tokens::Tokens128;
use ic_exports::ic_kit::RejectionCode;
use is20_token::account::{Account, Subaccount};
#[cfg(not(target_arch = "wasm32"))]
use is20_token::canister::TokenCanisterAPI;
use is20_token::state::ledger::{TransferArgs};
use is20_token_canister::canister::TokenCanister;

use crate::state::TokenTransferInfo;
use crate::state::PairError;
use crate::state::TokenConfiguration;

pub async fn get_icrc1_balance(token: Principal, account: Account) -> Result<Tokens128, PairError> {
    let canister = TokenCanister::from_principal(token);
    canister_call!(canister.icrc1_balance_of(account), Tokens128)
        .await
        .map_err(|e| PairError::TransactionMaybeFailed(token))
}

pub async fn transfer_icrc1(
    token: Principal,
    to: Account,
    amount: Tokens128,
    fee: Tokens128,
    from_subaccount: Option<Subaccount>,
) -> Result<TokenTransferInfo, PairError> {
    let token_canister = TokenCanister::from_principal(token);

    let args = TransferArgs {
        from_subaccount,
        to,
        amount,
        fee: Some(fee),
        memo: Default::default(),
        created_at_time: None,
    };

    let tx_id = canister_call!(
        token_canister.icrc1_transfer(args),
        Result<u128, is20_token::error::TransferError>
    )
    .await
    .map_err(|(code, e)| match code {
        // With these error codes IC guarantees that the target canister state is not changed, so the transaction is
        // not applied
        RejectionCode::DestinationInvalid
        | RejectionCode::CanisterReject
        | RejectionCode::CanisterError => PairError::TokenTransferFailed(e),
        // With other error codes we cannot be sure if the transaction is applied or not, so we must take special
        // care when dealing with these errors
        _ => PairError::TransactionMaybeFailed(token),
    })?
    .map_err(|e| PairError::TokenTransferFailed(e))?;

    Ok(TokenTransferInfo {
        token_tx_id: tx_id as u64,
        amount_transferred: amount,
        token_principal: token,
    })
}

pub async fn get_icrc1_configuration(token_principal: Principal) -> Option<TokenConfiguration> {
    let token_canister = TokenCanister::from_principal(token_principal);

    // ICRC-1 standard metadata doesn't include minting account, so we have to do two requests
    // to get both fields. It's fine though since this is done only one time.
    let fee = get_icrc1_fee(token_canister.clone()).await.ok()?;
    let minting_account = get_icrc1_minting_account(token_canister).await.ok()?;

    let minting_principal = match minting_account {
        Some(v) if v.subaccount.is_none() => v.owner,
        _ => Principal::management_canister(),
    };

    Some(TokenConfiguration {
        fee,
        minting_principal,
    })
}

pub async fn get_icrc1_fee(token: TokenCanister) -> Result<Tokens128, PairError> {
    canister_call!(token.icrc1_fee(), Tokens128)
        .await
        .map_err(|err| PairError::GenericError)
}

pub async fn get_icrc1_minting_account(token: TokenCanister) -> Result<Option<Account>, PairError> {
    canister_call!(token.icrc1_minting_account(), Option<Account>)
        .await
        .map_err(|err| PairError::GenericError)
}


pub async fn transfer(
    token: TokenCanister,
    to: Account,
    amount: Tokens128,
    from_subaccount: Option<Subaccount>,
) -> Result<TokenTransferInfo, PairError> {
    let principal = token.principal();
    let fee = get_icrc1_fee(token).await?;
    if let Some(amount_to_transfer) = amount - fee {
        transfer_icrc1(
                    principal,
                    to,
                    amount_to_transfer,
                    fee,
                    from_subaccount,
                )
                .await     
    } else {
        Err(PairError::NothingToTransfer)
    }
}