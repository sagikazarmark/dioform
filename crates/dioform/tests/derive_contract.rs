#[test]
fn derive_macro_compile_pass_contracts() {
    let tests = trybuild::TestCases::new();

    tests.pass("tests/derive_contract/pass/*.rs");
}

#[test]
fn derive_macro_compile_fail_contracts() {
    let tests = trybuild::TestCases::new();

    tests.compile_fail("tests/derive_contract/fail/*.rs");
}
