fn main() {
    ic_cdk::export::candid::export_service!();
    std::print!("{}", __export_service());
}
