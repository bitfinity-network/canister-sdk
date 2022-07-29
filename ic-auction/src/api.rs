use std::{cell::RefCell, rc::Rc};

use ic_canister::{update, Canister};
use ic_cdk::export::candid::Principal;
use ic_helpers::metrics::Interval;

use crate::error::{AuctionError, Result};
use crate::{AuctionInfo, AuctionState, BiddingInfo};

pub trait Auction: Canister + Sized {
    fn auction_state(&self) -> Rc<RefCell<AuctionState>>;

    fn canister_pre_update(&self, method_name: &str, _method_type: ic_canister::MethodType) {
        if method_name == "run_auction" {
            if !self.auction_state().borrow().bidding_state.is_auction_due() {
                ic_cdk::println!("Too early to begin auction");
            }
        } else {
            if let Err(auction_error) = self.run_auction() {
                ic_cdk::println!("Auction error: {auction_error:#?}");
            }
        }
    }

    fn disburse_rewards(&self) -> Result<AuctionInfo>;

    /// Starts the cycle auction.
    ///
    /// This method can be called only once in a [BiddingState.auction_period]. If the time elapsed
    /// since the last auction is less than the set period, [AuctionError::TooEarly] will be returned.
    ///
    /// The auction will distribute the accumulated fees in proportion to the user cycle bids, and
    /// then will update the fee ratio until the next auction.
    #[update(trait = true)]
    fn run_auction(&self) -> Result<AuctionInfo> {
        let auction_state = self.auction_state();

        if auction_state.borrow().bidding_state.bids.is_empty() {
            return Err(AuctionError::NoBids);
        }

        if !auction_state.borrow().bidding_state.is_auction_due() {
            return Err(AuctionError::TooEarlyToBeginAuction(
                auction_state
                    .borrow()
                    .bidding_state
                    .cooldown_secs_remaining(),
            ));
        }

        let result = self.disburse_rewards();

        auction_state.borrow_mut().reset_bidding_state();

        if let Ok(result) = result.clone() {
            println!("result ok");
            auction_state.borrow_mut().history.push(result);
        }

        result
    }

    /// Bid cycles for the next cycle auction.
    ///
    /// This method must be called with the cycles provided in the call. The amount of cycles cannot be
    /// less than 1_000_000. The provided cycles are accepted by the canister, and the user bid is
    /// saved for the next auction.
    #[update(trait = true)]
    fn bid_cycles(&self, bidder: Principal) -> Result<u64> {
        self.auction_state().borrow_mut().bid_cycles(bidder)
    }

    /// Current information about bids and auction.
    #[update(trait = true)]
    fn bidding_info(&self) -> BiddingInfo {
        self.auction_state().borrow().bidding_info()
    }

    /// Returns the information about a previously held auction.
    #[update(trait = true)]
    fn auction_info(&self, id: usize) -> Result<AuctionInfo> {
        self.auction_state().borrow_mut().auction_info(id)
    }

    /// Returns the minimum cycles set for the canister.
    ///
    /// This value affects the fee ratio set by the auctions. The more cycles available in the canister
    /// the less proportion of the fees will be transferred to the auction participants. If the amount
    /// of cycles in the canister drops below this value, all the fees will be used for cycle auction.
    #[update(trait = true)]
    fn get_min_cycles(&self) -> u64 {
        self.auction_state().borrow().min_cycles()
    }

    /// Update the controller of the auction.
    ///
    /// Only previous controller/owner is allowed to call this method.
    #[update(trait = true)]
    fn set_controller(&self, controller: Principal) -> Result<()> {
        self.auction_state()
            .borrow_mut()
            .authorize_owner()?
            .set_controller(controller);
        Ok(())
    }

    /// Sets the minimum cycles for the canister. For more information about this value, read [get_min_cycles].
    ///
    /// Only the owner is allowed to call this method.
    #[update(trait = true)]
    fn set_min_cycles(&self, min_cycles: u64) -> Result<()> {
        self.auction_state()
            .borrow_mut()
            .authorize_owner()?
            .set_min_cycles(min_cycles);
        Ok(())
    }

    /// Sets the minimum time between two consecutive auctions, in seconds.
    ///
    /// Only the owner is allowed to call this method.
    #[update(trait = true)]
    fn set_auction_period(&self, interval: Interval) -> Result<()> {
        let caller = ic_canister::ic_kit::ic::caller();
        println!("caller in: {}", caller);
        self.auction_state()
            .borrow_mut()
            .authorize_owner()?
            .set_auction_period(interval);
        Ok(())
    }

    // Important: This function *must* be defined to be the
    // last one in the trait because it depends on the order
    // of expansion of update/query(trait = true) methods.
    fn get_idl() -> ic_canister::Idl {
        ic_canister::generate_idl!()
    }
}
