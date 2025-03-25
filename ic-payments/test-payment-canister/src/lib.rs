use canister::PaymentCanister;

pub mod canister;

#[ic_canister::export_candid]
pub fn idl() -> String {
    let idl = PaymentCanister::idl();

    candid::pretty::candid::compile(&idl.env.env, &Some(idl.actor))
}
