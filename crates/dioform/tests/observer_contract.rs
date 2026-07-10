#[test]
fn observer_event_contract_rejects_exhaustive_matching() {
    let tests = trybuild::TestCases::new();

    tests.compile_fail("tests/observer_contract/fail/exhaustive_observer_matching.rs");
}
