use crate::canister::CounterCanister;

pub mod canister;

fn main() {
    let canister_e_idl = CounterCanister::idl();
    let idl =
        candid::bindings::candid::compile(&canister_e_idl.env.env, &Some(canister_e_idl.actor));

    println!("{}", idl);
}
