use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use ic_canister::{generate_idl, init, post_upgrade, Canister, Idl, PreUpdate};
use ic_exports::ic_cdk;
use ic_exports::ic_kit::ic;
use ic_log::canister::inspect::logger_canister_inspect;
use ic_log::canister::{LogCanister, LogState};
use ic_log::did::LogCanisterSettings;
use ic_stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{IcMemoryManager, MemoryId};
use ic_storage::IcStorage;

thread_local! {
    static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());
}

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

        MEMORY_MANAGER.with(|mm| {
            self.log_state()
                .borrow_mut()
                .init(ic::caller(), mm.get(MemoryId::new(1)), settings)
                .expect("error configuring the logger");
        });
    }

    #[post_upgrade]
    pub fn post_upgrade(&self) {
        MEMORY_MANAGER.with(|mm| {
            self.log_state()
                .borrow_mut()
                .reload(mm.get(MemoryId::new(1)))
                .expect("error configuring the logger");
        });
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
