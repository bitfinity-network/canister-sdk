use dfn_core::api::call_with_cleanup;
use dfn_protobuf::protobuf;
use ic_cdk::export::candid::{CandidType, Principal};
use ic_types::{CanisterId, PrincipalId};
use ledger_canister::{account_identifier::{AccountIdentifier, Subaccount}, tokens::Tokens, BlockHeight, BlockRes, Memo, Operation as Operate, SendArgs, TransferArgs, TRANSACTION_FEE, TransferError, BinaryAccountBalanceArgs};
use async_trait::async_trait;
use crate::ledger::account_id::FromPrincipal;

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

    #[deprecated(since="0.2.0", note="This method uses unstable ledger method. Use `transfer` instead")]
    async fn ledger_transfer(&self, to: Principal,  amount: u64, fee: u64) -> Result<u64, String>;
    async fn get_balance(&self, of: Principal, sub_account: Option<Subaccount>) -> Result<u64, String>;
    async fn transfer(&self, to: Principal, amount: u64, from_subaccount: Option<Subaccount>, to_subaccount: Option<Subaccount>) -> Result<u64, String>;
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
        if amount < fee {
            return Err(format!("Cannot transfer tokens. Amount `{}` is smaller then the fee `{}`.", amount, fee));
        }

        let args = SendArgs {
            memo: Memo(0x57444857),
            amount: (Tokens::from_e8s(amount) - Tokens::from_e8s(fee))?,
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

    async fn get_balance(&self, of: Principal, sub_account: Option<Subaccount>) -> Result<u64, String> {
        let account = AccountIdentifier::new(of.into(), sub_account);
        let args = BinaryAccountBalanceArgs { account: account.to_address() };
        let result = ic_cdk::call::<_, (Tokens,)>(*self, "account_balance", (args,)).await.map_err(|e| e.1)?.0;
        Ok(result.get_e8s())
    }

    async fn transfer(&self, to: Principal, amount: u64, from_subaccount: Option<Subaccount>, to_subaccount: Option<Subaccount>) -> Result<u64, String> {
        if amount < TRANSACTION_FEE.get_e8s() {
            return Err(format!("cannot transfer tokens: amount '{}' is smaller then the fee '{}'", amount, TRANSACTION_FEE.get_e8s()))
        }

        let args = TransferArgs {
            memo: Default::default(),
            amount: (Tokens::from_e8s(amount) - TRANSACTION_FEE)?,
            fee: TRANSACTION_FEE,
            from_subaccount,
            to: AccountIdentifier::from_principal(to, to_subaccount).to_address(),
            created_at_time: None
        };

        ic_cdk::call::<_, (Result<BlockHeight, TransferError>,)>(*self, "transfer", (args,)).await
            .map_err(|e| e.1)?
            .0
            .map_err(|e| format!("{e:?}"))
    }
}



