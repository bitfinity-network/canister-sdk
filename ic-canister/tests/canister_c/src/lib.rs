use candid::{CandidType, Deserialize, Principal};
use ic_helpers::metrics::Metrics;
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;
use std::{cell::RefCell, rc::Rc};

use ic_canister::{generate_exports, update, Canister, MethodType, PreUpdate};

#[derive(Default, CandidType, Deserialize, IcStorage)]
pub struct State {
    counter: u32,
}

impl Versioned for State {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

pub trait CanisterC: Canister + Metrics {
    fn state(&self) -> Rc<RefCell<State>> {
        State::get()
    }

    #[update(trait = true)]
    fn inc_counter(&mut self, value: u32) {
        RefCell::borrow_mut(&self.state()).counter += value;
    }
}

#[derive(CandidType, Deserialize, IcStorage, Default, Clone)]
pub struct MetricsSnapshot {
    pub cycles: u64,
}

#[derive(Clone, Canister)]
pub struct CanisterCExports {
    #[id]
    principal: Principal,
}

impl PreUpdate for CanisterCExports {
    fn pre_update(&self, _method_name: &str, _method_type: MethodType) {
        self.update_metrics();
    }
}

impl Metrics for CanisterCExports {}

impl CanisterC for CanisterCExports {}

generate_exports!(CanisterCExports);

#[cfg(test)]
mod tests {
    use super::*;
    use ic_canister::{canister_call, ic_kit::MockContext};

    #[tokio::test]
    async fn get_metrics() {
        let ctx = MockContext::new().inject();

        let mut canister_c = CanisterCExports::init_instance();

        let _ = canister_call!(canister_c.inc_counter(5), ()).await;

        ctx.add_time(6e+10 as u64 * 60 * 3); // 3 hours

        let _ = canister_call!(canister_c.inc_counter(5), ()).await;

        let metrics = canister_call!(canister_c.get_metrics(), ())
            .await
            .unwrap()
            .metrics;

        assert_eq!(metrics.map.len(), 2);

        let metrics_snapshot = metrics.map.into_iter().next().unwrap().1;

        assert_eq!(metrics_snapshot.cycles, 1e+14 as u64);
        assert_eq!(metrics_snapshot.stable_memory_size, 0);
    }
}
