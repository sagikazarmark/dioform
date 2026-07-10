use std::{fs, path::PathBuf};

#[test]
fn workspace_crates_keep_architecture_layers() {
    let core = manifest("dioform-core");
    assert_has_no_dependencies(
        "dioform-core",
        &core,
        &[
            "dioxus-core",
            "dioform",
            "dioform-derive",
            "dioxus-fullstack",
            "dioxus-server",
            "garde",
        ],
    );

    let facade = manifest("dioform");
    assert_has_dependencies(
        "dioform",
        &facade,
        &["dioxus-core", "dioform-core", "dioform-derive"],
    );
    assert_has_no_dependencies(
        "dioform",
        &facade,
        &["dioxus-fullstack", "dioxus-server", "garde"],
    );

    let derive = manifest("dioform-derive");
    assert_has_no_dependencies(
        "dioform-derive",
        &derive,
        &[
            "dioxus-core",
            "dioform",
            "dioform-core",
            "dioxus-fullstack",
            "dioxus-server",
            "garde",
        ],
    );

    let garde = manifest("dioform-garde");
    assert_has_dependencies(
        "dioform-garde",
        &garde,
        &["dioform-core", "dioform-validation-adapter", "garde"],
    );
    assert_has_no_dependencies(
        "dioform-garde",
        &garde,
        &[
            "dioxus-core",
            "dioform",
            "dioform-derive",
            "dioxus-fullstack",
            "dioxus-server",
        ],
    );

    let validation_adapter = manifest("dioform-validation-adapter");
    assert_has_dependencies(
        "dioform-validation-adapter",
        &validation_adapter,
        &["dioform-core"],
    );
    assert_has_no_dependencies(
        "dioform-validation-adapter",
        &validation_adapter,
        &[
            "dioxus-core",
            "dioform",
            "dioform-derive",
            "dioxus-fullstack",
            "dioxus-server",
            "garde",
            "validator",
        ],
    );

    let fullstack = manifest("dioform-fullstack");
    assert_has_dependencies(
        "dioform-fullstack",
        &fullstack,
        &[
            "dioxus-core",
            "dioform",
            "dioxus-fullstack",
            "dioxus-server",
        ],
    );
    assert_has_no_dependencies(
        "dioform-fullstack",
        &fullstack,
        &["dioform-core", "dioform-derive", "garde"],
    );
}

fn manifest(crate_name: &str) -> String {
    fs::read_to_string(
        workspace_root()
            .join("crates")
            .join(crate_name)
            .join("Cargo.toml"),
    )
    .expect("crate manifest should be readable")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("dioform crate should live under crates/")
        .to_path_buf()
}

fn assert_has_dependencies(crate_name: &str, manifest: &str, dependencies: &[&str]) {
    for dependency in dependencies {
        assert!(
            manifest_has_dependency(manifest, dependency),
            "{crate_name} should depend on {dependency}"
        );
    }
}

fn assert_has_no_dependencies(crate_name: &str, manifest: &str, dependencies: &[&str]) {
    for dependency in dependencies {
        assert!(
            !manifest_has_dependency(manifest, dependency),
            "{crate_name} should not depend on {dependency}"
        );
    }
}

fn manifest_has_dependency(manifest: &str, dependency: &str) -> bool {
    let dependency_prefix = format!("{dependency} =");

    manifest
        .lines()
        .map(str::trim_start)
        .any(|line| line.starts_with(&dependency_prefix))
}
