use ic_cdk::export::candid::{CandidType, Deserialize};
use thiserror::Error;

#[derive(Error, CandidType, Debug, Clone, Deserialize, PartialEq, Eq)]
pub enum AuctionError {
    #[error("provided cycles in the `bid_cycles` call is less then the minimum allowed amount")]
    BiddingTooSmall,

    #[error("there are no cycle bids pending, so the auction cannot be held")]
    NoBids,

    #[error("auction with the given id is not found")]
    AuctionNotFound,

    #[error("the specified period between the auctions is not passed yet. {0}s remaining")]
    TooEarlyToBeginAuction(u64),

    #[error("the principal {0} is not an auction controller")]
    Unauthorized(String),
}

pub type Result<T> = std::result::Result<T, AuctionError>;
