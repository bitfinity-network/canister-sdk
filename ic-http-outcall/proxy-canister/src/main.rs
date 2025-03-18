use ic_http_outcall_proxy_canister::HttpProxyCanister;

fn main() {
    let canister_e_idl = HttpProxyCanister::idl();
    let idl = candid::pretty::candid::compile(&canister_e_idl.env.env, &Some(canister_e_idl.actor));

    println!("{}", idl);
}
