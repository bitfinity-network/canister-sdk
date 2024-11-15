use std::cell::RefCell;
use std::rc::Rc;

use ic_canister::{generate_idl, update, Canister, MethodType, PreUpdate};
use ic_exports::candid::{CandidType, Deserialize, Principal};
use ic_metrics::{Metrics, MetricsStorage};
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;

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

#[derive(CandidType, Deserialize, IcStorage, Default, Clone)]
pub struct MetricsSnapshot {
    pub cycles: u64,
}

#[derive(Clone, Canister)]
pub struct CanisterC {
    #[id]
    principal: Principal,

    #[state]
    state: Rc<RefCell<State>>,
}

impl CanisterC {
    #[update]
    fn inc_counter(&mut self, value: u32) {
        self.state.borrow_mut().counter += value;
    }
}

impl Metrics for CanisterC {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        MetricsStorage::get()
    }
}

impl PreUpdate for CanisterC {
    fn pre_update(&self, _method_name: &str, _method_type: MethodType) {
        self.update_metrics();
    }
}

pub fn idl() -> String {
    use ic_canister::Idl;

    let canister_c_idl = generate_idl!();

    let mut metrics_idl = <CanisterC as Metrics>::get_idl();

    metrics_idl.merge(&canister_c_idl);

    candid::pretty::candid::compile(&metrics_idl.env.env, &Some(metrics_idl.actor))
}

#[cfg(test)]
mod tests {
    use ic_canister::canister_call;
    use ic_exports::ic_kit::MockContext;

    use super::*;

    #[tokio::test]
    async fn get_metrics() {
        let ctx = MockContext::new().inject();

        let mut canister_c = CanisterC::init_instance();

        let _ = canister_call!(canister_c.inc_counter(5), ()).await;

        ctx.add_time(6e+10 as u64 * 60 * 3); // 3 hours

        let _ = canister_call!(canister_c.inc_counter(5), ()).await;

        let metrics = canister_call!(canister_c.get_metrics(), ())
            .await
            .unwrap()
            .metrics;

        assert_eq!(metrics.map.len(), 2);

        let metrics_snapshot = metrics.map.into_iter().next().unwrap().1;

        assert_eq!(metrics_snapshot.cycles, 1e+14 as u128);
        assert_eq!(metrics_snapshot.stable_memory_size, 0);
    }
}
