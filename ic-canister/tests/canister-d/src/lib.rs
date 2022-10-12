use ic_canister::PreUpdate;
use ic_exports::ic_cdk::export::candid::Principal;
use std::cell::RefCell;

use ic_canister::{generate_exports, query, update, Canister};

thread_local! {
    pub static COUNTER: RefCell<u32> = RefCell::default();
}

// Canister trait with no `state_getter` method.
pub trait CanisterD: Canister {
    #[query(trait = true)]
    fn get_counter(&self) -> u32 {
        COUNTER.with(|c| *c.borrow())
    }

    #[update(trait = true)]
    fn inc_counter(&mut self, value: u32) {
        COUNTER.with(|c| *c.borrow_mut() += value)
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

generate_exports!(CanisterD, CanisterDImpl);

#[cfg(test)]
mod tests {
    use crate::{CanisterD, CanisterDImpl};
    use ic_canister::{canister_call, Canister};
    use ic_exports::ic_kit::MockContext;

    #[test]
    fn canister_works() {
        MockContext::new().inject();

        let mut canister = CanisterDImpl::init_instance();
        canister.inc_counter(3);

        assert_eq!(canister.get_counter(), 3);
        assert_eq!(
            CanisterDImpl::from_principal(canister.principal()).get_counter(),
            3
        );
    }

    #[tokio::test]
    async fn execution_context_with_canister_call() {
        let id = ic_exports::ic_kit::mock_principals::alice();
        let caller = ic_exports::ic_kit::mock_principals::bob();

        MockContext::new().with_id(id).with_caller(caller).inject();

        let canister = CanisterDImpl::init_instance();
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
