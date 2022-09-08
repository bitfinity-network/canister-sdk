use candid::Principal;
use cycles_minting_canister::{
    IcpXdrConversionRateCertifiedResponse, TokensToCycles, DEFAULT_CYCLES_PER_XDR,
};
use ic_base_types::{CanisterId, PrincipalId};
use ic_helpers::tokens::Tokens128;
use ledger_canister::{
    AccountIdentifier, CyclesResponse, NotifyCanisterArgs, SendArgs, Subaccount, Tokens,
    DEFAULT_TRANSFER_FEE,
};

use crate::error::FactoryError;

const CYCLE_MINTING_CANISTER_ID: &str = "rrkah-fqaaa-aaaaa-aaaaq-cai";

// This function get the `icp_xdr_conversion_rate` from the `cycles_minting_canister`
pub async fn get_conversion_rate() -> Result<IcpXdrConversionRateCertifiedResponse, FactoryError> {
    let principal = Principal::from_text(CYCLE_MINTING_CANISTER_ID).unwrap();
    let rate = ic_cdk::call::<_, (IcpXdrConversionRateCertifiedResponse,)>(
        principal,
        "get_icp_xdr_conversion_rate",
        ((),),
    )
    .await
    .map_err(|e| FactoryError::GenericError(e.1))?
    .0;

    Ok(rate)
}

// Convert Tokens to cycles
pub async fn tokens_to_cycles(amount: Tokens) -> Result<u64, FactoryError> {
    let rate = get_conversion_rate().await?;
    let cycles = TokensToCycles {
        xdr_permyriad_per_icp: rate.data.xdr_permyriad_per_icp,
        cycles_per_xdr: DEFAULT_CYCLES_PER_XDR.into(),
    };

    Ok(cycles.to_cycles(amount).into())
}

// Send DFX
pub async fn send_dfx(amount: u64, ledger: Principal) -> Result<u64, FactoryError> {
    let canister_minting_principal = Principal::from_text(CYCLE_MINTING_CANISTER_ID).unwrap();
    // Verify amount is greater than 2* DEFAULT_TRANSFER_FEE
    if amount < (2 * DEFAULT_TRANSFER_FEE.get_e8s()) {
        return Err(FactoryError::GenericError(format!(
            "cannot transfer tokens: amount '{}' is less then the fee '{}'",
            amount,
            DEFAULT_TRANSFER_FEE.get_e8s()
        )));
    };

    let canister_id = ic_canister::ic_kit::ic::id();
    let to = AccountIdentifier::new(
        PrincipalId::from(canister_minting_principal),
        Some(Subaccount::from(&PrincipalId::from(canister_id))),
    );

    let amount = amount - DEFAULT_TRANSFER_FEE.get_e8s();
    let amount = Tokens::from_e8s(amount);

    let args = SendArgs {
        memo: Default::default(),
        amount,
        fee: DEFAULT_TRANSFER_FEE,
        from_subaccount: None,
        to,
        created_at_time: None,
    };

    // Send DFX
    let block_height = ic_cdk::call::<_, (u64,)>(ledger, "send_dfx", (args,))
        .await
        .map_err(|e| {
            FactoryError::GenericError(format!(
                "failed to send DFX to cycles_minting_canister: {}",
                e.1
            ))
        })?
        .0;
    // Notify DFX
    notify_dfx(block_height, ledger).await?;

    let cycles = tokens_to_cycles(amount).await?;

    Ok(cycles)
}

pub async fn notify_dfx(block_height: u64, ledger: Principal) -> Result<(), FactoryError> {
    let canister_minting_principal = Principal::from_text(CYCLE_MINTING_CANISTER_ID).unwrap();
    let to_canister = CanisterId::new(PrincipalId::from(canister_minting_principal))
        .map_err(|e| FactoryError::GenericError(e.to_string()))?;

    let args = NotifyCanisterArgs {
        block_height,
        max_fee: DEFAULT_TRANSFER_FEE,
        from_subaccount: None,
        to_canister,
        to_subaccount: Some(Subaccount::from(&PrincipalId::from(
            ic_canister::ic_kit::ic::id(),
        ))),
    };
    let mut result: Option<CyclesResponse> = None;
    for _ in 0..5 {
        match ic_cdk::call::<_, (CyclesResponse,)>(ledger, "notify_dfx", (args.clone(),)).await {
            Ok((cycles,)) => {
                result = Some(cycles);
                break;
            }
            Err(_) => continue,
        }
    }
    let cycles = result.ok_or_else(|| {
        FactoryError::GenericError("failed to notify DFX to cycles_minting_canister".to_string())
    })?;

    match cycles {
        CyclesResponse::ToppedUp(_) => Ok(()),
        _ => Err(FactoryError::GenericError(
            "failed to notify DFX to cycles_minting_canister".to_string(),
        )),
    }
}
