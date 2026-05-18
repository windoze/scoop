//! Sentinel coverage checks for UMB helper-invariant buckets.

use std::collections::BTreeSet;

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
        "present-in-U6",
        "B-01 sentinel coverage should no longer be marked planned once U6 lands"
    );

    let inventory_entries = umb_inventory::inventory_entries();
    let b01_entries = inventory_entries
        .iter()
        .filter(|entry| entry.bucket == "B-01")
        .collect::<Vec<_>>();
    let b01_ids = b01_entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();
    let sentinel_ids = spec_coverage::sentinel_coverage_ids();

    assert_eq!(
        sentinel_ids, b01_ids,
        "B-01 sentinel coverage must enumerate exactly the helper-invariant ids"
    );
    for entry in b01_entries {
        assert_eq!(
            entry.spec_anchor, "N/A:helper-invariant",
            "{} should remain a helper-invariant entry",
            entry.id
        );
        assert_eq!(
            entry.expected_class, "InternalBugSentinel",
            "{} should remain an internal sentinel until P7 migrates the helper",
            entry.id
        );
    }
}
