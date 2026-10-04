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
        "runtime/src/image/lookup.c",
        "runtime/src/image/active.c",
        "runtime/src/image/scan_ranges.c",
        "runtime/src/image/types.c",
        "runtime/src/image/context_keys.c",
        "runtime/src/image/type_relations.c",
        "runtime/src/image/storage.c",
        "runtime/src/image/immortals.c",
        "runtime/src/image/static_values.c",
        "runtime/src/image/units.c",
        "runtime/src/image/allocation_ranges.c",
        "runtime/src/image/stackmaps.c",
        "runtime/src/gc/stackmap.c",
        "runtime/src/gc/stackmap/parser.c",
        "runtime/src/gc/stackmap/records.c",
        "runtime/src/gc/stackmap/fingerprint.c",
        "runtime/src/platform/image/darwin_sha256.c",
        "runtime/src/platform/arch/aarch64.c",
        "runtime/src/value_shape.c",
        "runtime/src/value_scan.c",
    ] {
        command.arg(workspace.join(source));
    }
    let compile = command
        .arg(workspace.join("runtime/tests/image_storage_fixture.c"))
        .arg(workspace.join("runtime/tests/image_stackmap_fixture.c"))
        .arg(workspace.join("runtime/tests/stackmap_fingerprint_test.c"))
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

#[test]
fn runtime_resolves_static_roots_immortals_and_initialization_state() {
    run_image_test(
        "image_storage_test",
        "image storage and initialization tests passed\n",
    );
}

#[test]
fn runtime_joins_complete_stackmaps_with_typed_sites_and_odr_bodies() {
    run_image_test("image_stackmap_test", "image stackmap join tests passed\n");
}
