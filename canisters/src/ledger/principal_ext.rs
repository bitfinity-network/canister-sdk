

use dfn_core::api::call_with_cleanup;
use dfn_protobuf::protobuf;
use ic_cdk::api::call::CallResult;
use ic_cdk::export::candid::{CandidType, Int, Nat, Principal};
use ic_types::{CanisterId, PrincipalId};
use ledger_canister::{
    account_identifier::{AccountIdentifier, Subaccount},
    tokens::Tokens, 
    BlockHeight, BlockRes, Memo, Operation as Operate, SendArgs,
};
use async_trait::async_trait;
use crate::ledger::account_id::{FromPrincipal, New};
use std::str::FromStr;

#[derive(CandidType, Debug, PartialEq)]
pub enum TxError {
    Unauthorized,
    AmountTooSmall,
    ErrorOperationStyle,
    ErrorTo,
    Other,
    InsufficientAllowance,
    InsufficientBalance,
}

#[derive(CandidType, Debug, PartialEq)]
pub struct Block{
    pub from: AccountIdentifier,
    pub to: AccountIdentifier,
    pub amount: u64,
    pub fee: u64
}

#[async_trait]
pub trait LedgerPrincipalExt {
    async fn fetch_block(&self, block_height: u64) -> Result<Block, String>;
    async fn verify_block(&self, caller: Principal, block: &Block, threshold: u64, sub_account: Option<Subaccount>) -> Result<(), TxError>;
    async fn ledger_transfer(&self, to: Principal,  amount: u64, fee: u64) -> Result<u64, String>;
}

#[async_trait]
impl LedgerPrincipalExt for Principal {

// res type has the from, to , amount, fee fields 
// verifies a block at a certain height 
async fn fetch_block(&self, block_height: u64) -> Result<Block, String>{
    let ledger_canister = CanisterId::new(PrincipalId(*self)).ok().unwrap();
    
    let BlockRes(res) = call_with_cleanup(ledger_canister, "block_pb", protobuf, block_height)
        .await
        .map_err(|e| format!("{:?}", e))?;

    let encode_block = if let Some(result_encode_block) = res {
        match result_encode_block {
            Ok(encode_block) => encode_block,
            Err(e) => {
                let storage = Principal::from_text(e.to_string()).map_err(|e| format!("{:?}", e))?;

                let storage_canister =
                    CanisterId::new(PrincipalId::from(storage)).map_err(|e| format!("{:?}", e))?;

                let BlockRes(res) =
                    call_with_cleanup(storage_canister, "get_block_pb", protobuf, block_height)
                        .await
                        .map_err(|e| format!("{:?}", e))?;

                res.ok_or("error")?.map_err(|e| format!("{:?}", e))?
            }
        }
    } else {
        return Err(String::from("Error"));
    };

    let block = encode_block.decode().map_err(|e| format!("{:?}", e))?;

    return match block.transaction.operation {
        Operate::Transfer{from,to,amount,fee,} => Ok(Block{from,to, amount: amount.get_e8s(), fee: fee.get_e8s()}),
        _ => Err(String::from("Error in operation style"))
    };
}

async fn verify_block(&self, 
    caller: Principal, 
    block: &Block, 
    threshold: u64, 
    sub_account: Option<Subaccount>) -> Result<(), TxError> {

        if AccountIdentifier::from_principal(caller, sub_account) != block.from {
            return Err(TxError::Unauthorized);
        }

        if AccountIdentifier::from_principal(*self, None) != block.to {
            println!("{:?}", block.to);
            return Err(TxError::ErrorTo);
        }

        if block.amount < threshold {
            return Err(TxError::AmountTooSmall);
        }

    Ok(())
}


async fn ledger_transfer(&self, to: Principal,  amount: u64, fee: u64) -> Result<u64, String> {
    
    let args = SendArgs {
        memo: Memo(0x57444857),
        amount: (Tokens::from_e8s(amount) - Tokens::from_e8s(fee)).unwrap(),
        fee: Tokens::from_e8s(fee),
        from_subaccount: None,
        to: AccountIdentifier::from_principal(to, None),
        created_at_time: None,
    };
    let result: Result<(u64,),_> = ic_cdk::call(*self,"send_dfx",(args,),).await;
    result
     .map(|v| v.0)
     .map_err(|e| format!("{:?}", e))

    }
}



