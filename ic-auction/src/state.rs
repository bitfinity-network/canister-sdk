use std::collections::HashMap;

use ic_exports::candid::{CandidType, Deserialize, Principal};
use ic_exports::ic_kit::ic;
use ic_helpers::tokens::Tokens128;
use ic_metrics::Interval;
use ic_storage::IcStorage;

use crate::error::{AuctionError, Result};

// Minimum bidding amount is required, for every update call costs cycles, and we want bidding
// to add cycles rather then to decrease them. 1M is chosen as one ingress call costs 590K cycles.
pub const MIN_BIDDING_AMOUNT: Cycles = 1_000_000;

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

//------------------------------------------------------------------------------
// Bidding state
//------------------------------------------------------------------------------

#[derive(CandidType, Debug, Clone, Deserialize)]
pub struct BiddingState {
    pub fee_ratio: f64,
    pub last_auction: Timestamp,
    pub auction_period: Timestamp,
    pub cycles_since_auction: Cycles,
    pub bids: HashMap<Principal, Cycles>,
}

impl BiddingState {
    pub fn is_auction_due(&self) -> bool {
        let curr_time = ic::time();
        let next_auction = self.last_auction + self.auction_period;
        curr_time >= next_auction
    }

    pub fn cooldown_secs_remaining(&self) -> u64 {
        let curr_time = ic::time();
        let next_auction = self.last_auction + self.auction_period;
        (next_auction - curr_time) / 1_000_000
    }
}

impl Default for BiddingState {
    fn default() -> Self {
        BiddingState {
            fee_ratio: 0.0,
            last_auction: ic::time(),
            auction_period: 10u64.pow(9) * 60 * 60 * 24, // 1 day
            cycles_since_auction: 0,
            bids: HashMap::new(),
        }
    }
}

//------------------------------------------------------------------------------
// Auction state
//------------------------------------------------------------------------------

#[derive(CandidType, Deserialize, IcStorage, Debug)]
pub struct AuctionState {
    pub bidding_state: BiddingState,
    pub history: Vec<AuctionInfo>,
    pub controller: Principal,
    min_cycles: Cycles,
}

impl Default for AuctionState {
    fn default() -> Self {
        AuctionState {
            controller: Principal::anonymous(),
            bidding_state: BiddingState::default(),
            history: Vec::new(),
            min_cycles: MIN_BIDDING_AMOUNT,
        }
    }
}

impl AuctionState {
    pub fn new(auction_period: Interval, controller: Principal) -> Self {
        AuctionState {
            controller,
            bidding_state: BiddingState {
                auction_period: auction_period.nanos(),
                ..Default::default()
            },
            history: Vec::new(),
            min_cycles: MIN_BIDDING_AMOUNT,
        }
    }

    pub fn authorize_owner(&mut self) -> Result<Authorized<Controller>> {
        let caller = ic_exports::ic_kit::ic::caller();
        if caller == self.controller {
            Ok(Authorized::<Controller<'_>> {
                auth: Controller { state: self },
            })
        } else {
            Err(AuctionError::Unauthorized(caller.to_string()))
        }
    }

    pub fn reset_bidding_state(&mut self) {
        self.bidding_state = BiddingState {
            fee_ratio: self.get_fee_ratio(),
            auction_period: self.bidding_state.auction_period,
            last_auction: ic::time(),
            ..Default::default()
        };
    }

    fn get_fee_ratio(&self) -> f64 {
        let min_cycles = self.min_cycles as f64;
        let current_cycles = ic::balance() as f64;
        if min_cycles == 0.0 {
            // Setting min_cycles to zero effectively turns off the auction functionality, as all the
            // fees will go to the owner.
            0.0
        } else if current_cycles <= min_cycles {
            1.0
        } else {
            // If current cycles are 10 times larger, then min_cycles, half of the fees go to the auction.
            // If current cycles are 1000 times larger, 17% of the fees go to the auction.
            2f64.powf((min_cycles / current_cycles).log10())
        }
    }

    pub fn bid_cycles(&mut self, bidder: Principal) -> Result<Cycles> {
        let amount = ic::msg_cycles_available();
        if amount < MIN_BIDDING_AMOUNT {
            return Err(AuctionError::BiddingTooSmall);
        }

        let amount_accepted = ic::msg_cycles_accept(amount);
        self.bidding_state.cycles_since_auction += amount_accepted;
        *self.bidding_state.bids.entry(bidder).or_insert(0) += amount_accepted;

        Ok(amount_accepted)
    }

    pub fn bidding_info(&self) -> BiddingInfo {
        BiddingInfo {
            fee_ratio: self.bidding_state.fee_ratio,
            last_auction: self.bidding_state.last_auction,
            auction_period: self.bidding_state.auction_period,
            total_cycles: self.bidding_state.cycles_since_auction,
            caller_cycles: self
                .bidding_state
                .bids
                .get(&ic::caller())
                .cloned()
                .unwrap_or(0),
        }
    }

    pub fn auction_info(&self, id: usize) -> Result<AuctionInfo> {
        self.history
            .get(id)
            .cloned()
            .ok_or(AuctionError::AuctionNotFound)
    }

    pub fn min_cycles(&self) -> Cycles {
        self.min_cycles
    }
}

/// A wrapper that helps us separate owner/caller methods with a
/// compile-time check that a non-owner cannot access them.
pub struct Authorized<T> {
    auth: T,
}

pub struct Controller<'a> {
    state: &'a mut AuctionState,
}

impl<'a> Authorized<Controller<'a>> {
    pub fn set_min_cycles(&mut self, min_cycles: Cycles) {
        self.auth.state.min_cycles = min_cycles;
    }

    pub fn set_auction_period(&mut self, interval: Interval) {
        self.auth.state.bidding_state.auction_period = interval.nanos();
    }

    pub fn set_controller(&mut self, controller: Principal) {
        self.auth.state.controller = controller;
    }
}
