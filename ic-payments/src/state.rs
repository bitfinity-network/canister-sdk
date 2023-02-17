use candid::{CandidType, Deserialize, Principal};
use ic_helpers::tokens::Tokens128;
use thiserror::Error;

pub type TxId = u64;

pub struct TokenTransferInfo {
    /// Transaction id returned by the token canister.
    pub token_tx_id: TxId,
    /// Principal of the transferred token.
    pub token_principal: Principal,
    /// Amount of tokens were transferred to the principal.
    pub amount_transferred: Tokens128,
}


#[derive(CandidType, Debug, Deserialize, Clone, Copy)]
pub struct TokenInfo {
    pub principal: Principal,
    pub configuration: Option<TokenConfiguration>,
}

#[derive(CandidType, Debug, Deserialize, Clone, Copy)]
pub struct TokenConfiguration {
    pub fee: Tokens128,
    pub minting_principal: Principal,
}


pub enum PairError {
    #[error("token {0} not in the pair")]
    TokenNotInPair(candid::Principal),

    #[error("{0}")]
    GenericError(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("insufficient tokens in transit")]
    InsufficientTransitTokens,

    #[error("insufficient liquidity to mint/burn {got} (minimum amount required is {expected})")]
    InsufficientLiquidity { expected: Tokens128, got: Tokens128 },

    #[error("transit amounts must be greater then 0 for mint operation, current transit amounts: ({0}, {1})")]
    MintZeroTransit(Tokens128, Tokens128),

    #[error("liquidity exceeds cap {0}. Cannot mint")]
    LiquidityExceedsCap(Tokens128),

    #[error("insufficient liquidity to mint/burn 0 (minimum amount required is 1)")]
    ZeroAmountToMintOrBurn,

    #[error("no tokens provided")]
    InsufficientSwapTokens,

    #[error("invalid amount in for swap: {0}")]
    InvalidSwapAmount(Tokens128),

    #[error("cannot swap two tokens at once")]
    MultiSwap,

    #[error("not enough liquidity in the canister")]
    InsufficientLiquidityToSwap,

    #[error("integer value is out of bounds")]
    IntegerOverflow,

    #[error("no tokens are available for transfer")]
    NothingToTransfer,

    #[error("swap amount:{0} is lower than the expected swap amount: {1}")]
    ExpectedSwapAmountLow(Tokens128, Tokens128),

    #[error("{0}")]
    InvalidArgument(String),

    #[error("token transfer operation failed: {0}")]
    TokenTransferFailed(TokenTransferError),

    #[error("token amount overflow")]
    AmountOverflow,

    #[error("sync or skim operations are still on cooldown")]
    OnCooldown,

    #[error("positive reserves expected")]
    ZeroReserves,

    #[error("positive token balance expected")]
    ZeroTokenBalance,

    #[error("error response from the canister {0}: {1}")]
    TransactionMaybeFailed(Principal, String),

    #[error("pair is in a locked state, cannot perform operation")]
    Locked,

    #[error("transaction {0} not found")]
    TransactionNotFound(u64),

    #[error("trading in the pair temporary disabled by the owner")]
    TradingDisabled,

    #[error("invariats check failed: {0}")]
    InvariantsCheck(String),
}
