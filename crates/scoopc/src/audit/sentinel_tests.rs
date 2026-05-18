//! Sentinel coverage checks for UMB helper-invariant buckets.

use super::spec_coverage;
use super::umb_inventory;

#[test]
fn umb_fix_helper_invariant_sentinel_tests_present() {
    let sentinel = spec_coverage::sentinel_marker_value("SENTINEL");
    assert!(
        sentinel.contains("crates/scoopc/src/audit/sentinel_tests.rs::umb_fix_helper_invariant_sentinel_tests_present"),
        "B-01 README should point at this sentinel test, got `{sentinel}`"
    );
    assert_eq!(
        spec_coverage::sentinel_marker_value("SENTINEL-STATUS"),
        "retired-by-P7-B1",
        "B-01 sentinel coverage should track retired helper-invariant ids after P7-B1"
    );

    let inventory_entries = umb_inventory::inventory_entries();
    let b01_entries = inventory_entries
        .iter()
        .filter(|entry| entry.bucket == "B-01")
        .collect::<Vec<_>>();
    assert!(
        b01_entries.is_empty(),
        "B-01 active inventory should be empty after P7-B1 helper migration"
    );
    let retired_b01_ids = umb_inventory::retired_ids_for_bucket("B-01");
    let sentinel_ids = spec_coverage::sentinel_coverage_ids();

    assert_eq!(
        sentinel_ids, retired_b01_ids,
        "B-01 sentinel coverage must enumerate exactly the retired helper-invariant ids"
    );
}
