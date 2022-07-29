/// Reason why the method may be accepted.
#[derive(Debug, Clone, Copy)]
pub enum AcceptReason {
    /// The call is a part of the auction API and can be performed.
    Valid,
    /// The method isn't a part of the auction API, and may require further validation.
    NotAuctionMethod,
}

pub fn inspect_message(method: &str, caller: Principal) -> Result<AcceptReason, &'static str> {
    match method {
        "runAuction" => {
            // We allow running auction only to the owner or any of the cycle bidders.
            let state = CanisterState::get();
            let state = state.borrow();
            let bidding_state = &state.bidding_state;
            if bidding_state.is_auction_due()
                && (bidding_state.bids.contains_key(&caller) || caller == state.stats.owner)
            {
                Ok(AcceptReason::Valid)
            } else {
                Err("Auction is not due yet or auction run method is called not by owner or bidder. Rejecting.")
            }
        }
        "bidCycles" => {
            // We reject this message, because a call with cycles cannot be made through ingress,
            // only from the wallet canister.
            Err("Call with cycles cannot be made through ingress environment.")
        }
        _ => Ok(AcceprReason::NotAuctionMethod),
    }
}
