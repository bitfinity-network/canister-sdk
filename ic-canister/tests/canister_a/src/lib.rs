use candid::{CandidType, Deserialize, Principal};
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;
use std::cell::RefCell;

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

#[derive(Clone, Canister)]
pub struct CanisterA {
    #[id]
    principal: Principal,
    #[state(stable_store = true)]
    state: std::rc::Rc<RefCell<State>>,
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
}
