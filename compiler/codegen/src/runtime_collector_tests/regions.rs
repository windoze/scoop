use super::{compile_and_run, workspace_root};

#[test]
fn region_growth_and_large_mapping_exceed_one_gibibyte() {
    let output = compile_and_run(
        &workspace_root(),
        "region_heap_test",
        "runtime/tests/region_heap_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"region growth and independent mappings exceed the former 1 GiB limit\n"
    );
}

#[test]
fn sparse_page_map_covers_uintptr_and_reclaims_empty_paths() {
    let output = compile_and_run(
        &workspace_root(),
        "page_map_test",
        "runtime/tests/page_map_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"sparse page map covers 64-bit keys and removes empty paths\n"
    );
}
