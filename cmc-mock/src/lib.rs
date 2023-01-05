use std::{cell::RefCell, rc::Rc};

use candid::{CandidType, Deserialize, Principal};
use ic_canister::{init, query, update, virtual_canister_call, Canister, PreUpdate};
use ic_exports::{
    cycles_minting_canister::{
        CyclesCanisterInitPayload, IcpXdrConversionRate, IcpXdrConversionRateCertifiedResponse,
        NotifyError, NotifyTopUp, TokensToCycles,
    },
    ic_kit::ic,
    ledger_canister::{CandidOperation, GetBlocksArgs, QueryBlocksResponse, Tokens},
    serde::Serialize,
    BlockHeight,
};
use ic_storage::{stable::Versioned, IcStorage};
use ic_types::Cycles;

#[derive(Debug, IcStorage, CandidType, Serialize, Deserialize)]
struct CmcState {
    xdr_permyriad_per_icp: u64,
    ledger: Principal,
}

impl Default for CmcState {
    fn default() -> Self {
        Self {
            xdr_permyriad_per_icp: Default::default(),
            ledger: Principal::anonymous(),
        }
    }
}

#[derive(Debug, Canister)]
struct CmcMockCanister {
    #[id]
    principal: Principal,

    #[state]
    state: Rc<RefCell<CmcState>>,
}

impl PreUpdate for CmcMockCanister {}
impl Versioned for CmcState {
    type Previous = ();

    fn upgrade(_previous: Self::Previous) -> Self {
        Self::default()
    }
}

impl CmcMockCanister {
    #[init]
    pub fn init(&mut self, payload: CyclesCanisterInitPayload) {
        self.state.borrow_mut().ledger = payload.ledger_canister_id.get().into();
    }

    #[query]
    pub fn get_icp_xdr_conversion_rate(&self) -> IcpXdrConversionRateCertifiedResponse {
        IcpXdrConversionRateCertifiedResponse {
            data: IcpXdrConversionRate {
                timestamp_seconds: ic::time(),
                xdr_permyriad_per_icp: self.state.borrow().xdr_permyriad_per_icp,
            },
            hash_tree: vec![],
            certificate: vec![],
        }
    }

    #[update]
    pub fn set_icp_xdr_conversion_rate(
        &mut self,
        payload: ic_nns_common::types::UpdateIcpXdrConversionRatePayload,
    ) -> Result<(), String> {
        self.state.borrow_mut().xdr_permyriad_per_icp = payload.xdr_permyriad_per_icp;
        Ok(())
    }

    #[update]
    pub async fn notify_top_up(&self, payload: NotifyTopUp) -> Result<u128, NotifyError> {
        let icp_amount = self.get_icp_block_amount(payload.block_index).await;
        let cycles_amount = TokensToCycles {
            xdr_permyriad_per_icp: self.state.borrow().xdr_permyriad_per_icp,
            cycles_per_xdr: Cycles::new(1_000_000_000_000),
        }
        .to_cycles(icp_amount)
        .get();

        send_cycles_to(payload.canister_id.get().into(), cycles_amount).await;

        Ok(cycles_amount)
    }

    async fn get_icp_block_amount(&self, block_height: BlockHeight) -> Tokens {
        let request = GetBlocksArgs {
            start: block_height,
            length: 1,
        };
        let ledger = self.state.borrow().ledger;
        let response =
            virtual_canister_call!(ledger, "query_blocks", (request,), QueryBlocksResponse)
                .await
                .unwrap();

        if response.blocks.len() == 1 {
            if let CandidOperation::Transfer { amount, .. } =
                response.blocks[0].transaction.operation
            {
                amount
            } else {
                panic!("Invalid ledger operation");
            }
        } else {
            panic!("Ledger block not found");
        }
    }
}

async fn send_cycles_to(canister_id: Principal, cycles_amount: u128) {
    #[derive(Debug, CandidType)]
    struct Args {
        canister_id: Principal,
    }

    let args = Args { canister_id };

    ic::call_with_payment(
        Principal::management_canister(),
        "deposit_cycles",
        (args,),
        cycles_amount as u64,
    )
    .await
    .unwrap()
}
