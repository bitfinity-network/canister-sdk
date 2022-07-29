use ic_cdk::export::candid::{CandidType, Deserialize};
use ic_helpers::tokens::Tokens128;

pub type Cycles = u64;
pub type TxId = u64;
pub type Timestamp = u64;

#[derive(CandidType, Debug, Clone, Deserialize, PartialEq)]
pub struct AuctionInfo {
    pub auction_id: usize,
    pub auction_time: Timestamp,
    pub tokens_distributed: Tokens128,
    pub cycles_collected: Cycles,
    pub fee_ratio: f64,
    pub first_transaction_id: TxId,
    pub last_transaction_id: TxId,
}

/// Current information about upcoming auction and current cycle bids.
#[derive(CandidType, Debug, Clone, Deserialize)]
pub struct BiddingInfo {
    /// Proportion of the transaction fees that will be distributed to the auction participants.
    ///
    /// The value of 1.0 means that all fees go to the auction, 0.0 means that all the fees go to
    /// the canister owner.
    pub fee_ratio: f64,

    /// Timestamp of the last auction.
    pub last_auction: Timestamp,

    /// Period of performing auctions. Auction cannot be started before `last_auction + auction_period`
    /// IC time.
    pub auction_period: Timestamp,

    /// Total cycles accumulated since the last auction.
    pub total_cycles: Cycles,

    /// The amount of cycles the caller bid for the upcoming auction.
    pub caller_cycles: Cycles,
}
