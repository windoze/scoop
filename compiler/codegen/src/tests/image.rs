fn run_image_test(fixture: &str, expected: &str) {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap();
    let binary = std::env::temp_dir().join(format!("scoop_{fixture}_{}", std::process::id()));
    let mut command = std::process::Command::new("cc");
    command.args(["-std=c11", "-Wall", "-Wextra", "-Werror"]);
    command.arg("-I").arg(workspace.join("runtime/include"));
    for source in [
        "runtime/src/image/ranges.c",
        "runtime/src/image/checks.c",
        "runtime/src/image/dependencies.c",
        "runtime/src/image/records.c",
        "runtime/src/image/registry.c",
        "runtime/src/image/scan_ranges.c",
        "runtime/src/image/types.c",
        "runtime/src/image/type_relations.c",
        "runtime/src/value_shape.c",
        "runtime/src/value_scan.c",
    ] {
        command.arg(workspace.join(source));
    }
    let compile = command
        .arg(workspace.join(format!("runtime/tests/{fixture}.c")))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let output = std::process::Command::new(&binary).output().unwrap();
    std::fs::remove_file(&binary).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn runtime_collects_complete_image_registrations_before_execution() {
    run_image_test(
        "image_registry_test",
        "image registry collection tests passed\n",
    );
}

#[test]
fn runtime_registers_type_code_and_scan_addresses_once() {
    run_image_test("image_type_test", "image type and scan tests passed\n");
}
