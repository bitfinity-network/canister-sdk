use ic_canister::PreUpdate;
use ic_exports::ic_cdk::export::candid::{CandidType, Deserialize, Principal};
use ic_storage::{stable::Versioned, IcStorage};
use std::{cell::RefCell, rc::Rc};

use ic_canister::{generate_exports, query, state_getter, update, Canister};

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

pub trait CanisterA: Canister {
    #[state_getter]
    fn state(&self) -> Rc<RefCell<StateA>>;

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
        ic_exports::ic_kit::ic::caller()
    }

    #[query(trait = true)]
    fn id(&self) -> Principal {
        ic_exports::ic_kit::ic::id()
    }
}

generate_exports!(CanisterA, CanisterAImpl);

#[cfg(test)]
mod tests {
    use super::*;
    use ic_canister::{canister_call, Canister};
    use ic_exports::ic_kit::MockContext;

    #[test]
    fn independent_states() {
        let ctx = MockContext::new().inject();

        let mut canister1 = CanisterAImpl::init_instance();
        let mut canister2 = CanisterAImpl::init_instance();

        ctx.update_id(canister1.principal());
        canister1.inc_counter(3);

        ctx.update_id(canister2.principal());
        canister2.inc_counter(5);

        ctx.update_id(canister1.principal());
        assert_eq!(canister1.get_counter(), 3);

        ctx.update_id(canister2.principal());
        assert_eq!(canister2.get_counter(), 5);

        ctx.update_id(canister1.principal());
        assert_eq!(
            CanisterAImpl::from_principal(canister1.principal()).get_counter(),
            3
        );

        ctx.update_id(canister2.principal());
        assert_eq!(
            CanisterAImpl::from_principal(canister2.principal()).get_counter(),
            5
        );
    }

    #[tokio::test]
    async fn execution_context_with_canister_call() {
        let id = ic_exports::ic_kit::mock_principals::alice();
        let caller = ic_exports::ic_kit::mock_principals::bob();

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

        assert_eq!(ic_exports::ic_kit::ic::id(), id);
        assert_eq!(ic_exports::ic_kit::ic::caller(), caller);
    }
}
