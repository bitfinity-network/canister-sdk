use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use ic_canister::{generate_idl, init, Canister, Idl, PreUpdate};
use ic_exports::ic_cdk;
use ic_exports::ic_kit::ic;
use ic_log::canister::inspect::logger_canister_inspect;
use ic_log::canister::{LogCanister, LogState};
use ic_log::did::LogCanisterSettings;
use ic_stable_structures::MemoryId;
use ic_storage::IcStorage;

#[derive(Canister)]
pub struct LoggerCanister {
    #[id]
    id: Principal,
}

impl PreUpdate for LoggerCanister {}

impl LogCanister for LoggerCanister {
    fn log_state(&self) -> Rc<RefCell<LogState>> {
        LogState::get()
    }
}

#[ic_cdk::inspect_message]
fn inspect() {
    logger_canister_inspect()
}

impl LoggerCanister {
    #[init]
    pub fn init(&self) {
        let settings = LogCanisterSettings {
            log_filter: Some("trace".into()),
            in_memory_records: Some(128),
            ..Default::default()
        };

        self.log_state()
            .borrow_mut()
            .init(ic::caller(), MemoryId::new(1), settings)
            .expect("error configuring the logger");
    }

    pub fn get_idl() -> Idl {
        generate_idl!()
    }
}

fn main() {
    let canister_idl = LoggerCanister::get_idl();
    let mut idl = <LoggerCanister as LogCanister>::get_idl();
    idl.merge(&canister_idl);

    let idl = candid::pretty::candid::compile(&idl.env.env, &Some(idl.actor));

    println!("{}", idl);
}
