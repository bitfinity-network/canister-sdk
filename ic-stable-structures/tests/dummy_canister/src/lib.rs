pub mod canister;

use canister::DummyCanister;

#[ic_canister::export_candid]
pub fn idl() -> String {
    let canister_e_idl = DummyCanister::idl();

    candid::pretty::candid::compile(&canister_e_idl.env.env, &Some(canister_e_idl.actor))
}
