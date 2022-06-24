use candid::{CandidType, Deserialize, Principal};
use ic_helpers::metrics::Metrics;
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;
use std::{cell::RefCell, rc::Rc};

use ic_canister::{generate_exports, query, update, Canister, MethodType, PreUpdate};

#[derive(Default, CandidType, Deserialize, IcStorage)]
pub struct StateA {
    counter: u32,
}

impl Versioned for StateA {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

pub trait CanisterA: Canister + Metrics {
    fn state(&self) -> Rc<RefCell<StateA>> {
        StateA::get()
    }

    /// Documentation for get_counter
    #[query(trait = true)]
    fn get_counter(&self) -> u32 {
        self.state().borrow().counter
    }

    #[update(trait = true)]
    fn inc_counter(&mut self, value: u32) {
        RefCell::borrow_mut(&self.state()).counter += value;
    }

    #[query(trait = true)]
    fn caller(&self) -> Principal {
        ic_canister::ic_kit::ic::caller()
    }

    #[query(trait = true)]
    fn id(&self) -> Principal {
        ic_canister::ic_kit::ic::id()
    }
}

#[derive(Clone, Canister)]
pub struct CanisterAImpl {
    #[id]
    principal: Principal,
}

impl PreUpdate for CanisterAImpl {
    fn pre_update(&self, _method_name: &str, _method_type: MethodType) {
        self.update_metrics();
    }
}

impl Metrics for CanisterAImpl {}

impl CanisterA for CanisterAImpl {}

generate_exports!(CanisterAImpl);

#[cfg(test)]
mod tests {
    use super::*;
    use ic_canister::canister_call;
    use ic_canister::ic_kit::MockContext;

    #[test]
    fn independent_states() {
        MockContext::new().inject();
        let curr_id = ic_canister::ic_kit::ic::id();

        let mut canister1 = CanisterAImpl::init_instance();
        let mut canister2 = CanisterAImpl::init_instance();

        assert_eq!(ic_canister::ic_kit::ic::id(), curr_id);

        canister1.inc_counter(3);
        canister2.inc_counter(5);

        assert_eq!(canister1.get_counter(), 3);
        assert_eq!(canister2.get_counter(), 5);

        assert_eq!(
            CanisterAImpl::from_principal(canister1.principal()).get_counter(),
            3
        );
        assert_eq!(
            CanisterAImpl::from_principal(canister2.principal()).get_counter(),
            5
        );
    }

    #[test]
    fn method_execution_context() {
        let id = ic_canister::ic_kit::mock_principals::alice();
        let caller = ic_canister::ic_kit::mock_principals::bob();

        MockContext::new().with_id(id).with_caller(caller).inject();

        let canister = CanisterAImpl::init_instance();
        assert_eq!(canister.caller(), id, "wrong caller");
        assert_eq!(canister.id(), canister.principal(), "wrong canister id");

        assert_eq!(ic_canister::ic_kit::ic::id(), id);
        assert_eq!(ic_canister::ic_kit::ic::caller(), caller);
    }

    #[tokio::test]
    async fn execution_context_with_canister_call() {
        let id = ic_canister::ic_kit::mock_principals::alice();
        let caller = ic_canister::ic_kit::mock_principals::bob();

        MockContext::new().with_id(id).with_caller(caller).inject();

        let canister = CanisterAImpl::init_instance();
        assert_eq!(
            canister_call!(canister.caller(), Principal).await.unwrap(),
            id,
            "wrong caller"
        );
        assert_eq!(
            canister_call!(canister.id(), Principal).await.unwrap(),
            canister.principal(),
            "wrong canister id"
        );

        assert_eq!(ic_canister::ic_kit::ic::id(), id);
        assert_eq!(ic_canister::ic_kit::ic::caller(), caller);
    }
}
