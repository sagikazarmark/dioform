#[test]
fn form_handles_can_be_dioxus_component_props() {
    let tests = trybuild::TestCases::new();

    tests.pass("tests/props_contract/pass/*.rs");
}
