# Cycle auction trait

As the IC canister must pay cycles for all operations it performs, as well as for its storage. It's
essential to make sure that the token canister always have enough cycles to run. One way to do it is to require the
canister owner to monitor the canister and top it up with cycles when needed. This approach, although simple, does not
allow the tokens to be fully decentralized.

This crate provides a mechanism of cycle auctions, that doesn't require owner's attention for the canister cycle management.

Cycle auctions are run in a set of intervals, and allow any user to add cycles to the canister and retrieve a reward set up via `disburse_rewards` call as a reward for doing so.

The main concepts of the mechanism are:

* `fee_ratio` is the proportion of the fees that will be distributed among the auction participants. This ratio is set
  at the end of each auction based on the current amount of cycles in the canister, and a `min_cycles` value, set by the
  owner. The ratio is `1.0` if the amount of cycles available is
  `min_cycles` or less, and exponentially decreases as the available amount of cycles increases. The value of `1.0`
  means that all the fees will be used for the next cycle auction, and the value of `0.5` means that half of the cycles
  will go to the owner while the other half will be used for the auction.
* `auction_period` - minimum period of time between two consecutive auctions. The default value is 1 day, but can be
  changed by the owner of the canister.
* `cycles_since_auction` - the transaction fees, collected since the last auction was held. This amount of cycles will be
  distributed at the next auction.

### Types

```
type Interval = variant {
  PerHour;
  PerWeek;
  PerDay;
  Period : record { seconds : nat64 };
  PerMinute;
};
type AuctionError = variant {
  NoBids;
  TooEarlyToBeginAuction : nat64;
  Unauthorized : text;
  BiddingTooSmall;
  AuctionNotFound;
};
type AuctionInfo = record {
  auction_time : nat64;
  auction_id : nat64;
  first_transaction_id : nat64;
  last_transaction_id : nat64;
  tokens_distributed : nat;
  cycles_collected : nat64;
  fee_ratio : float64;
};
type BiddingInfo = record {
  caller_cycles : nat64;
  auction_period : nat64;
  last_auction : nat64;
  total_cycles : nat64;
  fee_ratio : float64;
};
```

#### bid_cycles

Bid cycles for the next cycle auction.

This method must be called with the cycles provided in the call. The amount of cycles cannot be less than `MIN_BIDDING_AMOUNT`. The
provided cycles are accepted by the canister, and the user bid is saved for the next auction.

```
update bid_cycles : (bidder: principal) -> variant { Ok : nat64; Err: AuctionError };
```

### bidding_info

Current information about bids and auction.

```
query bidding_info() -> BiddingInfo;
```

### run_auction

Starts the cycle auction.

This method can be called only once in a `BiddingState::auction_period`. If the time elapsed since the last auction is
less than the set period, `AuctionError::TooEarlyToBeginAuction(seconds_remain)` will be returned.

The auction will distribute the accumulated fees in proportion to the user cycle bids, and then will update the fee
ratio until the next auction.

```
update run_auction() -> variant { Ok : AuctionInfo; Err: AuctionError }
```

### auction_info

Returns the information about previously held auction.

```
update auction_info(auction_id: nat32) -> variant { Ok : AuctionInfo; Err: AuctionError }
```

### get_min_cycles

Returns the minimum cycles set for the canister.

This value affects the fee ratio set by the auctions. The more cycles available in the canister the less proportion of
the fees will be transferred to the auction participants. If the amount of cycles in the canister drops below this
value, all the fees will be used for cycle auction.

```
query get_min_cycles() -> nat64
```

### set_min_cycles

Sets the minimum cycles for the canister. For more information about this value, read [get_min_cycles].

Only the owner is allowed to call this method.

```
update set_min_cycles(min_cycles: nat64) -> variant { Ok; Err: AuctionError }
```

### set_auction_period

Sets the minimum time between two consecutive auctions, in seconds.

Only the owner is allowed to call this method.

```
update set_auction_period(interval: Interval) -> variant { Ok; Err: AuctionError }
```

### set_controller 

Change the owner/controller of the auction.

Only the previous owner is allowed to call this methods.

```
set_controller : (new_controller: principal) -> variant { Ok; Err: AuctionError };
```
