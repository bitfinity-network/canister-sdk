use std::cell::RefCell;

use ic_canister::{generate_exports, generate_idl, query, update, Canister, Idl, PreUpdate};
use ic_exports::candid::Principal;
use ic_stable_structures::{
    get_memory_by_id, CellStructure, DefaultMemoryManager, DefaultMemoryResourceType,
    DefaultMemoryType, MemoryId, StableCell,
};

const MEMORY_ID: MemoryId = MemoryId::new(0);

thread_local! {
    pub static MEMORY_MANAGER: DefaultMemoryManager = DefaultMemoryManager::init(DefaultMemoryResourceType::default());

    pub static COUNTER: RefCell<StableCell<u32, DefaultMemoryType>> =
        RefCell::new(StableCell::new(get_memory_by_id(&MEMORY_MANAGER, MEMORY_ID), 0).expect("failed to initialize stable cell"));
}

// Canister trait with no `state_getter` method.
pub trait CanisterD: Canister {
    #[query(trait = true)]
    fn get_counter(&self) -> u32 {
        COUNTER.with(|c| *c.borrow().get())
    }

    #[update(trait = true)]
    fn inc_counter(&mut self, value: u32) {
        COUNTER
            .with(|c| {
                let new_value = c.borrow().get() + value;
                c.borrow_mut().set(new_value)
            })
            .expect("can't update cell value");
    }

    #[query(trait = true)]
    fn caller(&self) -> Principal {
        ic_exports::ic_kit::ic::caller()
    }

    #[query(trait = true)]
    fn id(&self) -> Principal {
        ic_exports::ic_kit::ic::id()
    }

    // Important: This function *must* be defined to be the
    // last one in the trait because it depends on the order
    // of expansion of update/query(trait = true) methods.
    fn get_idl() -> Idl {
        generate_idl!()
    }
}

generate_exports!(CanisterD, CanisterDImpl);

pub fn idl() -> String {
    let trait_idl = <CanisterDImpl as CanisterD>::get_idl();
    candid::bindings::candid::compile(&trait_idl.env.env, &Some(trait_idl.actor))
}

#[cfg(test)]
mod tests {
    
    use ic_canister::{canister_call, Canister};
    use ic_exports::ic_kit::MockContext;

    use crate::{CanisterD, CanisterDImpl};

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
