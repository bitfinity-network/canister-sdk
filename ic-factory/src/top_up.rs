use candid::Principal;
use cycles_minting_canister::{
    IcpXdrConversionRateCertifiedResponse, TokensToCycles, DEFAULT_CYCLES_PER_XDR,
    MEMO_TOP_UP_CANISTER,
};
use ic_base_types::{CanisterId, PrincipalId};
use ic_canister::virtual_canister_call;
use ledger_canister::{
    AccountIdentifier, CyclesResponse, NotifyCanisterArgs, SendArgs, Tokens, DEFAULT_TRANSFER_FEE,
};

use crate::error::FactoryError;

const CYCLE_MINTING_CANISTER: &str = "rrkah-fqaaa-aaaaa-aaaaq-cai";

async fn get_conversion_rate() -> Result<IcpXdrConversionRateCertifiedResponse, FactoryError> {
    let principal = Principal::from_text(CYCLE_MINTING_CANISTER).expect("const conversion");
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

async fn tokens_to_cycles(amount: Tokens) -> Result<u64, FactoryError> {
    let rate = get_conversion_rate().await?;
    let cycles = TokensToCycles {
        xdr_permyriad_per_icp: rate.data.xdr_permyriad_per_icp,
        cycles_per_xdr: DEFAULT_CYCLES_PER_XDR.into(),
    };

    let cycles: u64 = cycles.to_cycles(amount).into();
    // Actual cycles to be transferred is cycles -  fee(2_000_000_000)
    Ok(cycles - 2_000_000_000)
}

// Send DFX
pub async fn send_dfx_notify(
    amount: u64,
    ledger: Principal,
    caller: Principal,
) -> Result<u64, FactoryError> {
    let canister_minting_principal =
        Principal::from_text(CYCLE_MINTING_CANISTER).expect("const conversion");

    //  Twice the FEE because of `send_dfx` and `notify_dfx`
    let amount = if amount < (2 * DEFAULT_TRANSFER_FEE.get_e8s()) {
        Err(FactoryError::GenericError(format!(
            "cannot transfer tokens: amount '{}' is less then the fee '{}'",
            amount,
            2 * DEFAULT_TRANSFER_FEE.get_e8s()
        )))?
    } else {
        Tokens::from_e8s(amount - (DEFAULT_TRANSFER_FEE.get_e8s()))
    };

    let canister_id = ic_canister::ic_kit::ic::id();
    let to = AccountIdentifier::new(
        canister_minting_principal.into(),
        Some((&PrincipalId::from(canister_id)).into()),
    );

    let args = SendArgs {
        memo: MEMO_TOP_UP_CANISTER,
        amount,
        fee: DEFAULT_TRANSFER_FEE,
        from_subaccount: Some((&PrincipalId::from(caller)).into()),
        to,
        created_at_time: None,
    };

    let block_height = virtual_canister_call!(ledger, "send_dfx", (args,), u64)
        .await
        .map_err(|e| FactoryError::LedgerError(e.1))?;

    notify_dfx(block_height, ledger, canister_minting_principal, caller).await?;

    let cycles = tokens_to_cycles(amount).await?;

    Ok(cycles)
}

async fn notify_dfx(
    block_height: u64,
    ledger: Principal,
    minting_canister: Principal,
    caller: Principal,
) -> Result<(), FactoryError> {
    const MAX_RETRY: u64 = 5;

    let to_canister = CanisterId::new(minting_canister.into()).expect("const conversion");

    let args = NotifyCanisterArgs {
        block_height,
        max_fee: DEFAULT_TRANSFER_FEE,
        from_subaccount: Some((&PrincipalId::from(caller)).into()),
        to_canister,
        to_subaccount: Some((&PrincipalId::from(ic_canister::ic_kit::ic::id())).into()),
    };

    let mut result: Option<CyclesResponse> = None;

    for _ in 0..MAX_RETRY {
        match virtual_canister_call!(ledger, "notify_dfx", (args.clone(),), CyclesResponse).await {
            Ok(cycles) => {
                result = Some(cycles);
                break;
            }
            Err(_) => continue,
        }
    }

    if let Some(cycles) = result {
        match cycles {
            CyclesResponse::ToppedUp(_) => Ok(()),
            _ => Err(FactoryError::GenericError(
                "cycles response error".to_string(),
            )),
        }
    } else {
        Err(FactoryError::LedgerError("notify_dfx failed".to_string()))
    }
}
