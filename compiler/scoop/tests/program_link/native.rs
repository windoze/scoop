use super::*;

#[test]
fn ordinary_externs_share_runtime_contracts_and_use_actual_sdk_exports() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let library = environment.build(
        directory.path(),
        "native-library",
        "library",
        &fixture("native-library.scoop"),
        &[],
    );
    let root = environment.build(
        directory.path(),
        "native-root",
        "executable",
        &fixture("native-root.scoop"),
        &[("native-library", &library)],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let program = directory.path().join("program");
    environment.link(&root, &[&library], &program);
    for stress in [false, true] {
        assert_eq!(run(&program, stress), "42\nlibrary\nroot\n");
    }
}

#[test]
fn conflicting_unused_externs_report_every_cone_origin() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let left = environment.build(
        directory.path(),
        "left",
        "library",
        &fixture("native-conflict-left.scoop"),
        &[],
    );
    let right = environment.build(
        directory.path(),
        "right",
        "library",
        &fixture("native-conflict-right.scoop"),
        &[],
    );
    let root = environment.build(
        directory.path(),
        "root",
        "executable",
        &fixture("native-conflict-root.scoop"),
        &[("left", &left), ("right", &right)],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let error = rejected(environment, &root, &[&right, &left], directory.path());
    for expected in [
        "native contract conflict for _abs",
        "C parameter ABI differs",
        "dev.programlink:left",
        "dev.programlink:right",
        "dev.programlink:root",
        "native declarations",
    ] {
        assert!(error.contains(expected), "missing {expected}: {error}");
    }
}

#[test]
fn native_input_errors_do_not_replace_an_existing_executable() {
    let environment = environment();
    for (name, expected) in [
        (
            "native-missing",
            "unresolved native symbol _m23_missing_native",
        ),
        ("native-library-required", "missing native library"),
        ("native-effect-conflict", "GC effect differs"),
        (
            "native-kind-conflict",
            "incompatible function/data/TLS/mutability",
        ),
        (
            "native-reserved",
            "conflicts with a compiler-owned definition",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let root = environment.build(
            directory.path(),
            name,
            "executable",
            &fixture(&format!("{name}.scoop")),
            &[],
        );
        std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
        let error = rejected(environment, &root, &[], directory.path());
        assert!(error.contains(expected), "{name}: {error}");
        assert!(
            error.contains(&format!("dev.programlink:{name}")),
            "{error}"
        );
    }
}

fn rejected(
    environment: &Environment,
    root: &Path,
    dependencies: &[&Path],
    directory: &Path,
) -> String {
    let output = directory.join("existing-program");
    std::fs::write(&output, b"old executable").unwrap();
    let result = environment
        .link_command(root, dependencies, &output)
        .output()
        .unwrap();
    assert!(!result.status.success(), "link unexpectedly succeeded");
    assert_eq!(std::fs::read(output).unwrap(), b"old executable");
    String::from_utf8(result.stderr).unwrap()
}
