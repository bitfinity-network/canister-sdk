use ic_cdk::export::candid::{CandidType, Deserialize};
use thiserror::Error;

#[derive(Error, CandidType, Debug, Clone, Deserialize, PartialEq)]
pub enum AuctionError {
    #[error("Provided cycles in the `bid_cycles` call is less then the minimum allowed amount")]
    BiddingTooSmall,

    #[error("There are no cycle bids pending, so the auction cannot be held")]
    NoBids,

    #[error("Auction with the given ID is not found")]
    AuctionNotFound,

    #[error("The specified period between the auctions is not passed yet. {0}s remaining")]
    TooEarlyToBeginAuction(u64),

    #[error("The principal {0} is not an auction controller")]
    Unauthorized(String),
}

pub type Result<T> = std::result::Result<T, AuctionError>;
