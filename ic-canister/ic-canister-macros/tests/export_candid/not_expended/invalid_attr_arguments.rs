fn main() {}

#[ic_canister_macros::export_candid(attribute)]
fn did() -> String {
    panic!()
}
