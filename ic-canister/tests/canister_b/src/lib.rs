use candid::{CandidType, Deserialize, Principal};
use canister_a::CanisterAExports;
use ic_canister::{canister_call, canister_notify, virtual_canister_call, virtual_canister_notify};
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;
use std::cell::RefCell;
use std::rc::Rc;

use canister_a::CanisterA;

use ic_canister::{init, update, Canister};

#[derive(IcStorage, CandidType, Deserialize)]
struct State {
    canister_a: Principal,
}

impl Default for State {
    fn default() -> Self {
        Self {
            canister_a: Principal::anonymous(),
        }
    }
}

impl Versioned for State {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

#[derive(Clone, Canister)]
pub struct CanisterB {
    #[id]
    principal: Principal,
    #[state(stable = false)]
    state: Rc<RefCell<State>>,

    _another: u32,
}

impl CanisterB {
    #[init]
    fn init(&self, canister_a: Principal) {
        self.state.replace(State { canister_a });
    }

    #[update]
    #[allow(unused_mut)]
    async fn call_increment(&self, value: u32) -> u32 {
        let mut canister_a = CanisterAExports::from_principal(self.state.borrow().canister_a);

        canister_call!(canister_a.inc_counter(value), ())
            .await
            .unwrap();
        canister_call!(canister_a.get_counter(), u32).await.unwrap()
    }

    #[update]
    #[allow(unused_mut)]
    async fn call_increment_virtual(&self, value: u32) -> u32 {
        let mut canister_a = self.state.borrow().canister_a;

        virtual_canister_call!(canister_a, "inc_counter", (value,), ())
            .await
            .unwrap();
        virtual_canister_call!(canister_a, "get_counter", (), u32)
            .await
            .unwrap()
    }

    #[update]
    #[allow(unused_mut)]
    async fn notify_increment(&self, value: u32) -> bool {
        let mut canister_a = CanisterAExports::from_principal(self.state.borrow().canister_a);

        canister_notify!(canister_a.inc_counter(value), ()).unwrap();
        true
    }

    #[update]
    #[allow(unused_mut)]
    #[allow(unused_variables)]
    async fn notify_increment_virtual(&self, value: u32) -> bool {
        virtual_canister_notify!(self.state.borrow().canister_a, "inc_counter", (value,), ())
            .await
            .unwrap();
        true
    }

    #[query]
    async fn get_metrics_a(&self) -> MetricsMap<canister_a::Metrics> {
        let canister_a = CanisterA::from_principal(self.state.borrow().canister_a);

        canister_call!(canister_a.get_metrics(), MetricsMap<canister_a::Metrics>)
            .await
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ic_canister::ic_kit::MockContext;

    fn get_canister_b(canister_a: Principal) -> CanisterB {
        let canister = CanisterB::init_instance();
        canister.state.borrow_mut().canister_a = canister_a;

        canister
    }

    #[tokio::test]
    async fn inter_canister_call() {
        MockContext::new().inject();

        let canister_a = CanisterAExports::init_instance();
        let canister_a2 = CanisterAExports::init_instance();
        let canister_b = get_canister_b(canister_a.principal());
        let canister_b2 = get_canister_b(canister_a2.principal());

        assert_eq!(canister_b.call_increment(5).await, 5);
        assert_eq!(canister_b.call_increment(15).await, 20);
        assert_eq!(canister_b.notify_increment(20).await, true);
        assert_eq!(canister_a.__get_counter().await.unwrap(), 40);

        assert_eq!(canister_b2.notify_increment(100).await, true);
        assert_eq!(canister_a2.__get_counter().await.unwrap(), 100);
    }

    #[tokio::test]
    async fn get_metrics() {
        let ctx = ic_kit::MockContext::new().inject();

        let mut canister_a = CanisterA::init_instance();
        let canister_b = get_canister_b(canister_a.principal());

        canister_call!(canister_a.collect_metrics(), Result<()>)
            .await
            .unwrap();

        ctx.add_time(6u64.pow(10) * 60 * 3); // 3 hours

        canister_call!(canister_a.collect_metrics(), Result<()>)
            .await
            .unwrap();

        let metrics = canister_b.get_metrics_a().await;

        assert_eq!(metrics.map.len(), 2);
        assert_eq!(metrics.map.into_iter().next().unwrap().1.cycles, 100);
    }
}
