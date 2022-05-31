use candid::{CandidType, Deserialize, Principal};
use ic_helpers::metrics::MetricsMap;
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;
use std::{cell::RefCell, rc::Rc};

use ic_canister::{query, update, Canister};

#[derive(Default, CandidType, Deserialize, IcStorage)]
struct State {
    counter: u32,
}

impl Versioned for State {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

#[derive(IcStorage, Clone, Debug, Default)]

pub struct Metrics {
    pub cycles: u64,
}

#[derive(Clone, Canister)]
pub struct CanisterA {
    #[id]
    principal: Principal,
    #[state(stable_store = true)]
    state: Rc<RefCell<State>>,
    metrics: Rc<RefCell<MetricsMap<Metrics>>>,
}

impl CanisterA {
    #[query]
    fn get_counter(&self) -> u32 {
        self.state.borrow().counter
    }

    #[update]
    fn inc_counter(&mut self, value: u32) {
        RefCell::borrow_mut(&self.state).counter += value;
    }

    #[update]
    fn collect_metrics(&mut self) {
        let mut metrics = self.metrics.borrow_mut();
        metrics.insert(Metrics { cycles: 100 });
    }

    #[query]
    fn get_metrics(&self) -> MetricsMap<Metrics> {
        self.metrics.borrow().clone()
    }
}
