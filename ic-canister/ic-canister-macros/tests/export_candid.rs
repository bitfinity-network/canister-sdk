#[test]
fn not_expended() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/export_candid/not_expended/*.rs");
}

#[test]
fn expended() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/export_candid/expended/*.rs");
}
