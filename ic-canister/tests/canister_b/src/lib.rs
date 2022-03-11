use canister_a::CanisterA;
use ic_canister::canister_call;
use ic_cdk::export::Principal;
use ic_storage::IcStorage;
use std::cell::RefCell;
use std::rc::Rc;

use ic_canister::{update, Canister};

#[derive(IcStorage)]
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

#[export_name = "canister_init"]
fn init() {
    ::ic_cdk::setup();
    ::ic_cdk::block_on(async {
        let (canister_a,): (Principal,) = ic_cdk::api::call::arg_data();
        State::get().replace(State { canister_a });
    });
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
    #[update]
    async fn call_increment(&self, value: u32) -> u32 {
        let mut canister_a = CanisterA::from_principal(self.state.borrow().canister_a);

        canister_call!(canister_a.inc_counter(value), ())
            .await
            .unwrap();
        canister_call!(canister_a.get_counter(), u32).await.unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_canister_b(canister_a: Principal) -> CanisterB {
        let canister = CanisterB::init_instance();
        canister.state.borrow_mut().canister_a = canister_a;

        canister
    }

    #[tokio::test]
    async fn inter_canister_call() {
        let canister_a = CanisterA::init_instance();
        let canister_a2 = CanisterA::init_instance();
        let canister_b = get_canister_b(canister_a.principal());
        let canister_b2 = get_canister_b(canister_a2.principal());

        assert_eq!(canister_b.call_increment(5).await, 5);
        assert_eq!(canister_b.call_increment(15).await, 20);
        assert_eq!(canister_b2.call_increment(15).await, 15);
    }
}
