use candid::Principal;
use cycles_minting_canister::{
    IcpXdrConversionRateCertifiedResponse, NotifyError, NotifyTopUp, DEFAULT_CYCLES_PER_XDR,
    MEMO_TOP_UP_CANISTER,
};
use ic_base_types::{CanisterId, PrincipalId};
use ic_canister::virtual_canister_call;
use ledger_canister::{
    AccountIdentifier, SendArgs, Tokens, DEFAULT_TRANSFER_FEE, TOKEN_SUBDIVIDABLE_BY,
};
use num_traits::ToPrimitive;

use crate::error::FactoryError;

const CYCLE_MINTING_CANISTER: &str = "rkp4c-7iaaa-aaaaa-aaaca-cai";

/// This function calculates the amount required for minting cycles for a canister.
pub async fn calculate_amount(cycles: u64) -> Result<u64, FactoryError> {
    let rate = get_conversion_rate().await?.data;

    // Convert cycles to XDRs
    // 1 XDR = 10^12 cycles
    let xdr = cycles as f64 / DEFAULT_CYCLES_PER_XDR as f64;

    let one_icp = rate.xdr_permyriad_per_icp as f64 / 10_000.0;

    let icp = xdr / one_icp;

    (icp * TOKEN_SUBDIVIDABLE_BY as f64)
        .to_u64()
        .ok_or_else(|| FactoryError::GenericError("Failed to convert cycles to ICP".to_string()))
}

async fn get_conversion_rate() -> Result<IcpXdrConversionRateCertifiedResponse, FactoryError> {
    let principal = Principal::from_text(CYCLE_MINTING_CANISTER).expect("const conversion");

    let rate = virtual_canister_call!(
        principal,
        "get_icp_xdr_conversion_rate",
        (),
        IcpXdrConversionRateCertifiedResponse
    )
    .await
    .map_err(|e| FactoryError::GenericError(e.1))?;

    Ok(rate)
}

pub async fn send_dfx_notify(amount: u64, ledger: Principal) -> Result<u64, FactoryError> {
    let canister_minting_principal =
        Principal::from_text(CYCLE_MINTING_CANISTER).expect("const conversion");

    let canister_id = ic_canister::ic_kit::ic::id();
    let to = AccountIdentifier::new(
        canister_minting_principal.into(),
        Some((&PrincipalId::from(canister_id)).into()),
    );

    let args = SendArgs {
        memo: MEMO_TOP_UP_CANISTER,
        amount: Tokens::from_e8s(amount),
        fee: DEFAULT_TRANSFER_FEE,
        from_subaccount: None,
        to,
        created_at_time: None,
    };

    let block_height = virtual_canister_call!(ledger, "send_dfx", (args,), u64)
        .await
        .map_err(|e| FactoryError::LedgerError(e.1))?;

    let cycles = notify_top_up(block_height, canister_minting_principal).await?;

    Ok(cycles as u64)
}

async fn notify_top_up(
    block_height: u64,
    minting_canister: Principal,
) -> Result<u128, FactoryError> {
    let to_canister =
        CanisterId::new(ic_canister::ic_kit::ic::id().into()).expect("const conversion");

    let notify_details = NotifyTopUp {
        block_index: block_height,
        canister_id: to_canister,
    };

    let cycles = virtual_canister_call!(
        minting_canister,
        "notify_top_up",
        (notify_details,),
        Result<u128, NotifyError>
    )
    .await
    .map_err(|e| FactoryError::GenericError(e.1))?
    .map_err(|e| FactoryError::GenericError(e.to_string()))?;

    Ok(cycles)
}
