#[test]
fn listener_event_contract_rejects_exhaustive_matching() {
    let tests = trybuild::TestCases::new();

    tests.compile_fail("tests/listener_contract/fail/exhaustive_listener_event_matching.rs");
}
