//! Metrics for the canister can be collected by adding a specific state field to the canister struct
//!
//! ```ignore
//! #[derive(IcStorage, Clone, Debug, Default)]
//! pub struct Metrics {
//!     pub cycles: u64,
//! }
//!
//! #[derive(Clone, Canister)]
//! struct MyCanister {
//!     #[id]
//!     principal: Principal,
//!     
//!     metrics: std::rc::Rc<RefCell<ic_helpers::MetricsMap<Metrics>>>,
//!
//!     #[state]
//!     state: Rc<RefCell<MyCanisterState>>,
//! }
//! ```
//!
//! Note that `Metrics` is wrapped inside the [`MetricsMap`] wrapper. This is an implementation
//! detail that is used for storing metric snapshots. Under the hood this is a wrapper over
//! a `BTreeMap` with the keys being timestamps over some specified time interval. The user
//! should not be bothered about modifying the elements of that map, only setting up the logic
//! for metric snapshot calculation. This is as simple as calling an `insert()` method of the `MetricsMap`.
//!
//! For example, to actually store and get metrics the user has to define two endpoints
//!
//! ```ignore
//! #[update]
//! fn collect_metrics(&mut self) {
//!     let mut metrics = self.metrics.borrow_mut();
//!     metrics.insert(Metrics { cycles: ic_kit::ic::balance() });
//! }
//!
//! #[query]
//! fn get_metrics(&self) -> MetricsMap<Metrics> {
//!     self.metrics.borrow().clone()
//! }
//! ```
//!
//! And that's it. The metrics are stored at each `time + metrics.interval`, that is defined in the `MetricsMap`
//! struct. If the user decides to collect metrics before the time interval was passed, then the metric gets
//! overwritten.
//!
//! For the further example you can refer to the tests in the `canister-b` crate.
pub mod map;
pub use map::*;