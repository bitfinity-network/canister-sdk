use candid::Principal;
use ic_canister::virtual_canister_call;
use cycles_minting_canister::{
    IcpXdrConversionRateCertifiedResponse, NotifyError, NotifyTopUp, DEFAULT_CYCLES_PER_XDR,
    MEMO_TOP_UP_CANISTER,
};
use ic_exports::ic_base_types::{CanisterId, PrincipalId};
use ic_exports::ledger::{
    AccountIdentifier, Subaccount, Tokens, TransferArgs, TransferError, DEFAULT_TRANSFER_FEE,
    TOKEN_SUBDIVIDABLE_BY,
};
use ic_exports::BlockHeight;

use crate::error::FactoryError;

/// This Principal is a slice equivalent to `rkp4c-7iaaa-aaaaa-aaaca-cai`.
pub(crate) const CYCLES_MINTING_CANISTER: Principal =
    Principal::from_slice(&[0, 0, 0, 0, 0, 0, 0, 4, 1, 1]);

/// Calculates amount of ICP that can be converted to the given amount of cycles
pub async fn icp_amount_from_cycles(cmc: Principal, cycles: u64) -> Result<u64, FactoryError> {
    let rate = get_conversion_rate(cmc).await?.data;

    if rate.xdr_permyriad_per_icp == 0 {
        return Err(FactoryError::GenericError(
            "XDR permyriad per ICP is 0".to_string(),
        ));
    }

    Ok(calculate_icp(cycles, rate.xdr_permyriad_per_icp))
}

fn calculate_icp(cycles: u64, xdr_permyriad_per_icp: u64) -> u64 {
    (cycles as u128 * TOKEN_SUBDIVIDABLE_BY as u128 * 10_000
        / DEFAULT_CYCLES_PER_XDR
        / xdr_permyriad_per_icp as u128) as u64
}

async fn get_conversion_rate(
    cmc: Principal,
) -> Result<IcpXdrConversionRateCertifiedResponse, FactoryError> {
    virtual_canister_call!(
        cmc,
        "get_icp_xdr_conversion_rate",
        (),
        IcpXdrConversionRateCertifiedResponse
    )
    .await
    .map_err(|e| FactoryError::GenericError(e.1))
}

pub(crate) async fn transfer_icp_to_cmc(
    cmc: Principal,
    amount: u64,
    ledger: Principal,
    caller_subaccount: Subaccount,
) -> Result<BlockHeight, FactoryError> {
    let canister_id = ic_exports::ic_cdk::id();
    let to = AccountIdentifier::new(cmc.into(), Some((&PrincipalId::from(canister_id)).into()))
        .to_address();

    let args = TransferArgs {
        memo: MEMO_TOP_UP_CANISTER,
        amount: Tokens::from_e8s(amount - DEFAULT_TRANSFER_FEE.get_e8s()),
        fee: DEFAULT_TRANSFER_FEE,
        from_subaccount: Some(caller_subaccount),
        to,
        created_at_time: None,
    };

    virtual_canister_call!(ledger, "transfer", (args,), Result<BlockHeight, TransferError>)
        .await
        .map_err(|e| FactoryError::LedgerError(e.1))?
        .map_err(|e| FactoryError::LedgerError(format!("{:?}", e)))
}

pub(crate) async fn mint_cycles_to_factory(
    cmc: Principal,
    block_height: BlockHeight,
) -> Result<u128, FactoryError> {
    let to_canister =
        CanisterId::new(ic_exports::ic_kit::ic::id().into()).expect("const conversion");

    let notify_details = NotifyTopUp {
        block_index: block_height,
        canister_id: to_canister,
    };

    virtual_canister_call!(
        cmc,
        "notify_top_up",
        (notify_details,),
        Result<u128, NotifyError>
    )
    .await
    .map_err(|e| FactoryError::GenericError(e.1))?
    .map_err(|e| FactoryError::GenericError(format!("{:?}", e)))
}

#[cfg(test)]
mod tests {
    use ic_canister::register_virtual_responder;
    use cycles_minting_canister::IcpXdrConversionRate;
    use ic_exports::ic_kit::MockContext;

    use super::*;

    #[tokio::test]
    async fn test_calculate_amount() {
        MockContext::new().inject();

        register_virtual_responder(
            CYCLES_MINTING_CANISTER,
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
            (3_000_000_000_000, 61761436),
        ];

        let expected_icp = icp_amount_from_cycles(CYCLES_MINTING_CANISTER, cycles_icp[0].0)
            .await
            .unwrap();
        assert_eq!(expected_icp, cycles_icp[0].1);
        let expected_icp = icp_amount_from_cycles(CYCLES_MINTING_CANISTER, cycles_icp[1].0)
            .await
            .unwrap();
        assert_eq!(expected_icp, cycles_icp[1].1);
        let expected_icp = icp_amount_from_cycles(CYCLES_MINTING_CANISTER, cycles_icp[2].0)
            .await
            .unwrap();
        assert_eq!(expected_icp, cycles_icp[2].1);
        let expected_icp = icp_amount_from_cycles(CYCLES_MINTING_CANISTER, cycles_icp[3].0)
            .await
            .unwrap();
        assert_eq!(expected_icp, cycles_icp[3].1);
    }

    #[tokio::test]
    async fn test_mint_cycles() {
        MockContext::new().inject();
        register_virtual_responder(CYCLES_MINTING_CANISTER, "notify_top_up", |()| {
            Ok::<u128, NotifyError>(1_000_000_000_000)
        });
        let block_height = 100;
        let cycles = mint_cycles_to_factory(CYCLES_MINTING_CANISTER, block_height)
            .await
            .unwrap();
        assert_eq!(cycles, 1_000_000_000_000);
    }
}
