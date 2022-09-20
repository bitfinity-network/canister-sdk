use candid::Principal;
use cycles_minting_canister::{
    IcpXdrConversionRateCertifiedResponse, NotifyError, NotifyTopUp, DEFAULT_CYCLES_PER_XDR,
    MEMO_TOP_UP_CANISTER,
};
use ic_base_types::{CanisterId, PrincipalId};
use ic_canister::virtual_canister_call;
use ledger_canister::{
    AccountIdentifier, BlockHeight, SendArgs, Subaccount, Tokens, DEFAULT_TRANSFER_FEE,
    TOKEN_SUBDIVIDABLE_BY,
};
use num_traits::ToPrimitive;

use crate::error::FactoryError;

const CYCLE_MINTING_CANISTER: &str = "rkp4c-7iaaa-aaaaa-aaaca-cai";

/// This function calculates the amount required for minting cycles for a canister.
pub async fn cycles_to_icp(cycles: u64) -> Result<u64, FactoryError> {
    let rate = get_conversion_rate().await?.data;

    let icp_per_xdr = 10_000.0 / rate.xdr_permyriad_per_icp as f64;

    // Convert cycles to XDRs - 1 XDR = 10^12 cycles
    let xdr = cycles as f64 / DEFAULT_CYCLES_PER_XDR as f64;

    let icp = xdr * icp_per_xdr;

    (icp * TOKEN_SUBDIVIDABLE_BY as f64)
        .to_u64()
        .ok_or_else(|| FactoryError::GenericError("Failed to convert cycles to ICP".to_string()))
}

async fn get_conversion_rate() -> Result<IcpXdrConversionRateCertifiedResponse, FactoryError> {
    let principal = Principal::from_text(CYCLE_MINTING_CANISTER).expect("const conversion");

    virtual_canister_call!(
        principal,
        "get_icp_xdr_conversion_rate",
        (),
        IcpXdrConversionRateCertifiedResponse
    )
    .await
    .map_err(|e| FactoryError::GenericError(e.1))
}

pub(crate) async fn transfer_icp_to_cmc(
    amount: u64,
    ledger: Principal,
    caller_subaccount: Subaccount,
) -> Result<BlockHeight, FactoryError> {
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
        from_subaccount: Some(caller_subaccount),
        to,
        created_at_time: None,
    };

    virtual_canister_call!(ledger, "send_dfx", (args,), u64)
        .await
        .map_err(|e| FactoryError::LedgerError(e.1))
}

pub(crate) async fn mint_cycles_to_factory(
    block_height: BlockHeight,
) -> Result<u128, FactoryError> {
    let minting_canister = Principal::from_text(CYCLE_MINTING_CANISTER).expect("const conversion");
    let to_canister =
        CanisterId::new(ic_canister::ic_kit::ic::id().into()).expect("const conversion");

    let notify_details = NotifyTopUp {
        block_index: block_height,
        canister_id: to_canister,
    };

    virtual_canister_call!(
        minting_canister,
        "notify_top_up",
        (notify_details,),
        Result<u128, NotifyError>
    )
    .await
    .map_err(|e| FactoryError::GenericError(e.1))?
    .map_err(|e| FactoryError::GenericError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cycles_minting_canister::IcpXdrConversionRate;
    use ic_canister::{ic_kit::MockContext, register_virtual_responder};

    #[tokio::test]
    async fn test_calculate_amount() {
        register_virtual_responder(
            Principal::from_text(CYCLE_MINTING_CANISTER).unwrap(),
            "get_icp_xdr_conversion_rate",
            |()| IcpXdrConversionRateCertifiedResponse {
                data: IcpXdrConversionRate {
                    xdr_permyriad_per_icp: 48574,
                    timestamp_seconds: 1663144200,
                },
                hash_tree: vec![],
                certificate: vec![],
            },
        );

        let cycles_icp = vec![
            (5_000_000_000_000, 102935726),
            (1_000_000_000_000, 20587145),
            (2_000_000_000_000, 41174290),
            (3_000_000_000_000, 61761436), // off by one? 61761435
        ];

        let expected_icp = cycles_to_icp(cycles_icp[0].0).await.unwrap();
        assert_eq!(expected_icp, cycles_icp[0].1);
        let expected_icp = cycles_to_icp(cycles_icp[1].0).await.unwrap();
        assert_eq!(expected_icp, cycles_icp[1].1);
        let expected_icp = cycles_to_icp(cycles_icp[2].0).await.unwrap();
        assert_eq!(expected_icp, cycles_icp[2].1);
        let expected_icp = cycles_to_icp(cycles_icp[3].0).await.unwrap();
        assert_eq!(expected_icp, cycles_icp[3].1);
    }

    #[tokio::test]
    async fn test_mint_cycles() {
        MockContext::new().inject();
        register_virtual_responder(
            Principal::from_text(CYCLE_MINTING_CANISTER).unwrap(),
            "notify_top_up",
            |()| Ok::<u128, NotifyError>(1_000_000_000_000),
        );
        let block_height = 100;
        let cycles = mint_cycles_to_factory(block_height).await.unwrap();
        assert_eq!(cycles, 1_000_000_000_000);
    }
}
