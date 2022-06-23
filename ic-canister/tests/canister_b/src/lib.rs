use candid::{CandidType, Deserialize, Principal};
use canister_a::{CanisterA, CanisterAImpl};
use ic_canister::{
    canister_call, canister_notify, query, virtual_canister_call, virtual_canister_notify,
    CanisterBase,
};
use ic_helpers::metrics::{Metrics, MetricsMap};
use ic_storage::IcStorage;
use std::cell::RefCell;
use std::rc::Rc;

use ic_canister::{init, update, Canister};

#[derive(IcStorage, CandidType, Deserialize)]
struct StateB {
    canister_a: Principal,
}

impl Default for StateB {
    fn default() -> Self {
        Self {
            canister_a: Principal::anonymous(),
        }
    }
}

#[derive(Clone, Canister)]
#[canister_trait_name(Canister)]
#[canister_no_upgrade_methods]
pub struct CanisterB {
    #[id]
    principal: Principal,
    #[state]
    state: Rc<RefCell<StateB>>,

    _another: u32,
}

impl CanisterBase for CanisterB {}

impl Metrics for CanisterB {
    type MetricsStruct = ic_helpers::metrics::DefaultMetrics; // associated type defaults are unstable
}

impl CanisterB {
    #[init]
    fn init(&self, canister_a: Principal) {
        self.state.replace(StateB { canister_a });
    }

    #[update]
    #[allow(unused_mut)]
    async fn call_increment(&self, value: u32) -> u32 {
        let mut canister_a = CanisterAImpl::from_principal(self.state.borrow().canister_a);

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
        let mut canister_a = CanisterAImpl::from_principal(self.state.borrow().canister_a);

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

    #[update]
    async fn ids(&self) -> (Principal, Principal) {
        let canister_a = CanisterAImpl::from_principal(self.state.borrow().canister_a);
        let canister_a_id = canister_call!(canister_a.id(), Principal).await.unwrap();

        (ic_canister::ic_kit::ic::id(), canister_a_id)
    }

    #[update]
    async fn callers(&self) -> (Principal, Principal) {
        let canister_a = CanisterAImpl::from_principal(self.state.borrow().canister_a);
        let canister_a_caller = canister_call!(canister_a.caller(), Principal)
            .await
            .unwrap();

        (ic_canister::ic_kit::ic::caller(), canister_a_caller)
    }

    #[query]
    async fn get_metrics_a(&self) -> MetricsMap<canister_a::MetricsSnapshot> {
        let canister_a = CanisterAImpl::from_principal(self.state.borrow().canister_a);

        canister_call!(
            canister_a.get_metrics_(),
            MetricsMap<canister_a::MetricsSnapshot>
        )
        .await
        .unwrap()
    }
}

impl CanisterA for CanisterB {}

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

        let canister_a = CanisterAImpl::init_instance();
        let canister_a2 = CanisterAImpl::init_instance();
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
    async fn inter_canister_context() {
        let id = ic_canister::ic_kit::mock_principals::alice();
        let caller = ic_canister::ic_kit::mock_principals::bob();
        MockContext::new().with_id(id).with_caller(caller).inject();

        let canister_a = CanisterAImpl::init_instance();
        let canister_b = CanisterB::init_instance();
        canister_b.init(canister_a.principal());

        assert_eq!(
            canister_b.ids().await.0,
            canister_b.principal(),
            "invalid canister_b principal"
        );
        assert_eq!(
            canister_b.ids().await.1,
            canister_a.principal(),
            "invalid canister_a principal"
        );

        assert_eq!(
            canister_b.callers().await.0,
            id,
            "invalid canister_b caller"
        );
        assert_eq!(
            canister_b.callers().await.1,
            canister_b.principal(),
            "invalid canister_a caller"
        );
    }

    #[tokio::test]
    async fn trait_methods() {
        MockContext::new().inject();
        let mut canister = CanisterB::init_instance();

        canister.inc_counter(13);
        canister.inc_counter(2);
        assert_eq!(canister.get_counter(), 15);

        canister_call!(canister.inc_counter(3), ()).await.unwrap();
        assert_eq!(
            canister_call!(canister.get_counter(), u32).await.unwrap(),
            18
        );
    }

    #[tokio::test]
    async fn get_metrics() {
        let ctx = MockContext::new().inject();

        let canister_a = CanisterAImpl::init_instance();
        let canister_b = get_canister_b(canister_a.principal());

        let _ = canister_b.call_increment(5).await;

        ctx.add_time(6 * 10u64.pow(9) * 60 * 3); // 3 hours

        let _ = canister_b.call_increment(5).await;

        let metrics = canister_b.get_metrics_a().await;

        assert_eq!(metrics.map.len(), 2);
        assert_eq!(metrics.map.into_iter().next().unwrap().1.cycles, 100);
    }
}
