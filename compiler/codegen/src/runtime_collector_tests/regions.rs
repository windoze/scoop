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

#[test]
fn sparse_regions_compact_into_live_holes_and_shrink_around_pins() {
    let output = compile_and_run(
        &workspace_root(),
        "region_compaction_test",
        "runtime/tests/region_compaction_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"sparse regions compact into live holes and shrink across pinned growth cycles\n"
    );
}

#[test]
fn empty_pages_preserve_cross_page_pins_and_retry_failed_discard() {
    let output = compile_and_run(
        &workspace_root(),
        "page_discard_test",
        "runtime/tests/page_discard_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"discard preserves cross-page pins, retries failure and reuses zeroed storage\n"
    );
}

#[test]
fn full_reservation_failure_keeps_roots_and_avoids_empty_cache_migration() {
    let output = compile_and_run(
        &workspace_root(),
        "evacuation_fallback_test",
        "runtime/tests/evacuation_fallback_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"full reservation failure preserves roots and skips unprofitable cache migration\n"
    );
}
