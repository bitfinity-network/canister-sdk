pub mod canister;

use canister::DummyCanister;

fn main() {
    let canister_e_idl = DummyCanister::idl();
    let idl = candid::pretty::candid::compile(&canister_e_idl.env.env, &Some(canister_e_idl.actor));

    println!("{}", idl);
}
