#[test]
fn non_parsed_binding_contracts_exclude_parse_concepts() {
    let tests = trybuild::TestCases::new();

    tests.compile_fail("tests/binding_contract/fail/*.rs");
}
