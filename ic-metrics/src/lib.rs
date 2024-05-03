//! Metrics for the canister can be collected by adding a specific state field to the canister struct
//!
//! ```
//! use candid::Principal;
//! use ic_canister::{Canister, MethodType, PreUpdate};
//! use ic_metrics::*;
//! use ic_storage::IcStorage;
//! use std::cell::RefCell;
//!
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
//!     metrics: std::rc::Rc<RefCell<MetricsMap<Metrics>>>,
//! }
//!
//! impl PreUpdate for MyCanister {
//!     fn pre_update(&self, _method_name: &str, _method_type: MethodType) {}
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

use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use ic_canister::{generate_exports, generate_idl, query, state_getter, Canister, Idl, PreUpdate};
use ic_exports::candid::{CandidType, Deserialize};
use ic_storage::IcStorage;

#[cfg(target_family = "wasm")]
const WASM_PAGE_SIZE: u64 = 65536;

#[derive(CandidType, Deserialize, IcStorage, Default, Clone, Debug)]
pub struct MetricsStorage {
    pub metrics: MetricsMap<MetricsData>,
}

#[derive(CandidType, Deserialize, IcStorage, Default, Clone, Debug, PartialEq, Eq)]
pub struct MetricsData {
    pub cycles: u64,
    pub stable_memory_size: u64,
    pub heap_memory_size: u64,
}

pub trait Metrics: Canister {
    #[state_getter]
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>>;

    #[query(trait = true)]
    fn get_curr_metrics(&self) -> MetricsData {
        curr_values()
    }

    #[query(trait = true)]
    fn get_metrics(&self) -> MetricsStorage {
        MetricsStorage::get().borrow().clone()
    }

    fn update_metrics(&self) {
        let metrics = MetricsStorage::get();
        let mut metrics = metrics.borrow_mut();
        metrics.metrics.insert(curr_values());
    }

    /// This function updates the metrics at intervals with the specified timer
    ///
    /// This function is only available for the wasm target and won't do
    /// anything on other targets
    fn update_metrics_timer(&mut self, timer: std::time::Duration) {
        if cfg!(target_family = "wasm") {
            use ic_exports::ic_cdk_timers;
            let metrics = MetricsStorage::get();

            // Set the interval
            let interval = Interval::from_secs(timer.as_secs());
            metrics.borrow_mut().metrics.interval = interval;

            ic_cdk_timers::set_timer_interval(timer, move || {
                metrics.borrow_mut().metrics.insert(curr_values());
            });
        }
    }

    fn set_interval(interval: Interval) {
        MetricsStorage::get().borrow_mut().metrics.interval = interval;
    }

    // Important: This function *must* be defined to be the
    // last one in the trait because it depends on the order
    // of expansion of update/query(trait = true) methods.
    // This function generates the candid bindings for the Metrics trait
    fn get_idl() -> Idl {
        generate_idl!()
    }
}

fn curr_values() -> MetricsData {
    MetricsData {
        cycles: ic_exports::ic_kit::ic::balance(),
        stable_memory_size: {
            #[cfg(target_family = "wasm")]
            {
                ic_exports::ic_cdk::api::stable::stable64_size()
            }
            #[cfg(not(target_family = "wasm"))]
            {
                0
            }
        },
        heap_memory_size: {
            #[cfg(target_family = "wasm")]
            {
                (core::arch::wasm32::memory_size(0) as u64) * WASM_PAGE_SIZE
            }
            #[cfg(not(target_family = "wasm"))]
            {
                0
            }
        },
    }
}

#[derive(Debug, Copy, Clone, CandidType, Deserialize)]
pub enum Interval {
    PerMinute,
    PerHour,
    PerDay,
    PerWeek,
    Period { seconds: u64 },
}

impl Interval {
    pub fn nanos(&self) -> u64 {
        match self {
            Interval::Period { seconds } => *seconds * 1e+9 as u64,
            Interval::PerMinute => 60 * 1e+9 as u64,
            Interval::PerHour => 60 * 60 * 1e+9 as u64,
            Interval::PerDay => 24 * 60 * 60 * 1e+9 as u64,
            Interval::PerWeek => 7 * 24 * 60 * 60 * 1e+9 as u64,
        }
    }

    pub fn from_secs(secs: u64) -> Self {
        Interval::Period { seconds: secs }
    }
}

#[derive(Clone, CandidType, Deserialize, Debug)]
pub struct MetricsMap<T: IcStorage> {
    interval: Interval,
    history_length_nanos: u64,
    pub map: std::collections::BTreeMap<u64, T>,
}

impl<T: IcStorage> MetricsMap<T> {
    pub fn new(interval: Interval, history_length_nanos: u64) -> Self {
        Self {
            interval,
            history_length_nanos,
            map: std::collections::BTreeMap::new(),
        }
    }

    pub fn get_interval(&self) -> Interval {
        self.interval
    }

    pub fn insert(&mut self, new_metric: T) -> Option<T> {
        self.trim();
        let current_ts = ic_exports::ic_kit::ic::time();
        let last_ts = self
            .map
            .iter()
            .next_back()
            .map(|(k, _)| *k)
            .unwrap_or(current_ts);
        let new_ts = if current_ts < last_ts + self.interval.nanos() {
            last_ts
        } else {
            current_ts - (current_ts % self.interval.nanos())
        };
        self.map.insert(new_ts, new_metric)
    }

    fn trim(&mut self) {
        let current_ts = ic_exports::ic_kit::ic::time();
        let oldest_to_keep = current_ts.saturating_sub(self.history_length_nanos);
        self.map.retain(|&ts, _| ts >= oldest_to_keep);
    }
}

impl<T: IcStorage> std::default::Default for MetricsMap<T> {
    fn default() -> Self {
        Self::new(Interval::PerHour, Interval::PerDay.nanos() * 365)
    }
}

generate_exports!(Metrics);
