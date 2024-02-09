use crate::canister::CounterCanister;

pub mod canister;

fn main() {
    let canister_e_idl = CounterCanister::idl();
    let idl = candid::pretty::candid::compile(&canister_e_idl.env.env, &Some(canister_e_idl.actor));

    println!("{}", idl);
}
