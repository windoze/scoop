use super::{compile_and_run, workspace_root};

#[test]
fn bitmap_sweep_preserves_boundary_starts_pins_and_release() {
    let output = compile_and_run(
        &workspace_root(),
        "sweep_bitmap_test",
        "runtime/tests/sweep_bitmap_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"bitmap sweep preserves boundary starts, pins and exactly-once release after compaction\n"
    );
}

#[test]
fn insufficient_full_capacity_skips_target_sweep_and_retries_after_reclamation() {
    let output = compile_and_run(
        &workspace_root(),
        "full_capacity_test",
        "runtime/tests/full_capacity_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"full capacity rejection precedes target sweep and permits later compaction\n"
    );
}

#[test]
fn resolved_forwarding_rejects_invalid_collection_metadata() {
    for (verify, stress) in [(false, false), (true, false), (false, true)] {
        let output = super::compile::compile_and_run_configuration(
            &workspace_root(),
            "forwarding_validation_test",
            "runtime/tests/forwarding_validation_test.c",
            verify,
            stress,
            &["-O2"],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"mark boundaries and forwarding verification reject invalid references\n"
        );
    }
}

#[test]
fn region_growth_and_large_mapping_exceed_one_gibibyte() {
    for (verify, stress, optimization) in [(true, true, "-O0"), (false, false, "-O2")] {
        let output = super::compile::compile_and_run_configuration(
            &workspace_root(),
            "region_heap_test",
            "runtime/tests/region_heap_test.c",
            verify,
            stress,
            &[optimization],
        );
        assert!(
            output.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"region growth and independent mappings exceed the former 1 GiB limit\n"
        );
    }
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
