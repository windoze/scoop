//! Test-only builder and checker for `audit/UMB_inventory.csv`.
//!
//! The inventory is derived from the source tree instead of maintained by hand:
//! every `LlvmEmitError::UnsupportedMainBody { ... }` constructor under
//! `crates/scoopc/src/llvm/codegen/**` gets a stable `UMB-NNNN` id and
//! governance metadata used by later audit phases.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const CODEGEN_ROOT: &str = "crates/scoopc/src/llvm/codegen";
pub(crate) const INVENTORY_PATH: &str = "audit/UMB_inventory.csv";
pub(crate) const INITIAL_INVENTORY_PATH: &str = "audit/UMB_inventory_initial.csv";
pub(crate) const RETIRED_LEDGER_PATH: &str = "audit/UMB_retired.csv";
pub(crate) const INITIAL_ENTRY_COUNT: usize = 1_284;
pub(crate) const CSV_HEADER: &str = "id,file,line,kind,route,surface,bucket,expected_class,spec_anchor,upstream_gate,existing_fixture,notes";
pub(crate) const RETIRED_LEDGER_HEADER: &str =
    "id,bucket,expected_class,file,old_line,kind,retired_by,retired_reason,retired_at_notes";
#[cfg(test)]
const EXPECTED_CLASSES: &[&str] = &["FrontendReject", "InternalBugSentinel", "RealImpl"];

pub(crate) const VALID_BUCKETS: &[&str] = &[
    "B-01", "B-02", "B-03", "B-04", "B-05", "B-06", "B-07", "B-08", "B-09", "B-10", "B-11", "B-12",
    "B-13", "B-14", "B-15", "B-16", "B-17", "B-18", "B-19", "B-20", "B-21", "B-22", "B-23", "B-24",
    "B-25", "B-26", "B-27", "B-28", "B-29", "B-30", "B-31", "B-32", "B-33", "B-34", "B-35", "B-36",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KindSource {
    Literal,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
enum StableIdMode {
    Strict,
    Diff,
}

#[derive(Debug, Clone)]
struct SourceEntry {
    file: String,
    line: usize,
    column: usize,
    kind: String,
    kind_source: KindSource,
}

#[derive(Debug, Clone)]
pub(crate) struct InventoryEntry {
    pub(crate) id: String,
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) kind: String,
    route: &'static str,
    surface: &'static str,
    pub(crate) bucket: &'static str,
    pub(crate) expected_class: &'static str,
    pub(crate) spec_anchor: &'static str,
    upstream_gate: &'static str,
    existing_fixture: &'static str,
    notes: String,
    kind_source: KindSource,
}

#[derive(Debug, Clone)]
struct InventorySnapshotEntry {
    id: String,
    file: String,
    line: usize,
    kind: String,
    surface: String,
    bucket: String,
    expected_class: String,
}

#[derive(Debug, Clone)]
struct RetiredEntry {
    id: String,
    bucket: String,
    expected_class: String,
    file: String,
    old_line: usize,
    kind: String,
    retired_by: String,
    retired_reason: String,
    retired_at_notes: String,
}

struct Classification {
    bucket: &'static str,
    expected_class: &'static str,
    spec_anchor: &'static str,
    upstream_gate: &'static str,
    notes: String,
}

#[test]
fn umb_inventory_csv_in_sync() {
    let entries = inventory_entries();
    validate_inventory_entries(&entries);

    let csv = render_csv(&entries);
    let path = repo_root().join(INVENTORY_PATH);
    if std::env::var_os("SCOOP_WRITE_UMB_INVENTORY").is_some() {
        fs::write(&path, &csv)
            .unwrap_or_else(|err| panic!("failed to write {}: {err}", path.display()));
    }

    let on_disk = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    assert_eq!(
        on_disk, csv,
        "{INVENTORY_PATH} is out of sync; rerun with SCOOP_WRITE_UMB_INVENTORY=1"
    );

    println!(
        "UMB inventory: {} entries, {} literal kind fields, {} dynamic kind fields",
        entries.len(),
        entries
            .iter()
            .filter(|entry| entry.kind_source == KindSource::Literal)
            .count(),
        entries
            .iter()
            .filter(|entry| entry.kind_source == KindSource::Dynamic)
            .count()
    );
    println!("bucket distribution:\n{}", bucket_distribution(&entries));
}

#[test]
fn umb_inventory_diff_generation_matches_strict_inventory_when_in_sync() {
    assert_eq!(
        render_csv(&inventory_entries_for_diff()),
        render_csv(&inventory_entries()),
        "diff-mode inventory generation should match strict generation when no drift exists"
    );
}

#[test]
fn umb_inventory_kind_counts_match_source() {
    let source_entries = scan_source_entries();
    let inventory = inventory_entries();
    let source_counts = literal_kind_counts(
        source_entries
            .iter()
            .filter(|entry| entry.kind_source == KindSource::Literal)
            .map(|entry| entry.kind.as_str()),
    );
    let inventory_counts = literal_kind_counts(
        inventory
            .iter()
            .filter(|entry| entry.kind_source == KindSource::Literal)
            .map(|entry| entry.kind.as_str()),
    );

    assert_eq!(source_counts, inventory_counts);
    assert_eq!(
        source_counts.values().sum::<usize>(),
        inventory_counts.values().sum::<usize>(),
        "literal kind count should track the current active inventory"
    );
    let source_dynamic_count = source_entries
        .iter()
        .filter(|entry| entry.kind_source == KindSource::Dynamic)
        .count();
    let inventory_dynamic_count = inventory
        .iter()
        .filter(|entry| entry.kind_source == KindSource::Dynamic)
        .count();
    assert_eq!(
        source_dynamic_count, inventory_dynamic_count,
        "dynamic kind count should track the current active inventory"
    );
    assert_eq!(
        source_entries.len(),
        inventory.len(),
        "source scan and active inventory should have the same current active count"
    );
}

#[test]
fn umb_inventory_cross_references_legacy_gap_buckets() {
    let entries = inventory_entries();
    let retired = read_retired_ledger();
    let observed = entries
        .iter()
        .flat_map(|entry| legacy_gap_ids_from_notes(&entry.notes))
        .chain(
            retired
                .iter()
                .flat_map(|entry| legacy_gap_ids_from_notes(&entry.retired_at_notes)),
        )
        .collect::<BTreeSet<_>>();
    let expected = LEGACY_GAP_BUCKETS
        .iter()
        .map(|(gap_id, _)| *gap_id)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        observed, expected,
        "every legacy CODEGEN_GAP_INVENTORY entry should be represented in inventory notes"
    );
}

#[test]
fn umb_inventory_buckets_total() {
    let entries = inventory_entries();
    validate_inventory_entries(&entries);
    let counts = bucket_counts(&entries);
    let mut documented_total = 0usize;

    for &bucket in VALID_BUCKETS {
        let documented = bucket_doc_entry_count(bucket);
        let observed = counts.get(bucket).copied().unwrap_or_default();
        assert_eq!(
            observed, documented,
            "{bucket} entry count drift: inventory has {observed}, bucket doc declares {documented}"
        );
        documented_total += documented;
    }

    assert_eq!(
        documented_total,
        entries.len(),
        "bucket docs should sum to the current active UMB inventory count"
    );
}

#[test]
fn umb_inventory_each_entry_has_spec_anchor_or_helper_marker() {
    let entries = inventory_entries();
    validate_inventory_entries(&entries);

    for entry in entries {
        let anchor = entry.spec_anchor.trim();
        if anchor == "N/A:helper-invariant" {
            assert_eq!(
                entry.bucket, "B-01",
                "{} uses helper marker outside B-01",
                entry.id
            );
            assert_eq!(
                entry.expected_class, "InternalBugSentinel",
                "{} helper marker must remain an internal sentinel",
                entry.id
            );
            continue;
        }

        assert!(
            anchor.contains("docs/spec/language_spec-part"),
            "{} has non-helper spec_anchor `{anchor}` that does not point at the language spec",
            entry.id
        );
        for spec_anchor in anchor.split(';') {
            let Some((spec_path, fragment)) = spec_anchor.split_once('#') else {
                panic!(
                    "{} spec_anchor `{spec_anchor}` is missing a # fragment",
                    entry.id
                );
            };
            assert!(
                !fragment.trim().is_empty(),
                "{} spec_anchor `{spec_anchor}` has an empty fragment",
                entry.id
            );
            assert!(
                repo_root().join(spec_path).is_file(),
                "{} spec_anchor `{spec_anchor}` points at a missing spec file",
                entry.id
            );
        }
    }
}

#[test]
fn umb_inventory_class_distribution() {
    let entries = inventory_entries();
    validate_inventory_entries(&entries);
    let bucket_counts = bucket_counts(&entries);

    for &bucket in VALID_BUCKETS {
        let documented = bucket_doc_class_counts(bucket);
        let mut observed = BTreeMap::new();
        for entry in entries.iter().filter(|entry| entry.bucket == bucket) {
            *observed.entry(entry.expected_class).or_insert(0usize) += 1;
        }

        let documented_total = documented.values().copied().sum::<usize>();
        assert_eq!(
            documented_total,
            bucket_counts.get(bucket).copied().unwrap_or_default(),
            "{bucket} Expected Post-Fix Class table should sum to the inventory bucket count"
        );

        let d_pending = documented.get("D-pending").copied().unwrap_or_default();
        if d_pending > 0 {
            assert_eq!(
                bucket, "B-36",
                "only the spec-uncovered bucket may use D-pending in this plan"
            );
            assert_eq!(
                observed.get("FrontendReject").copied().unwrap_or_default(),
                d_pending,
                "{bucket} D-pending rows must currently be represented as FrontendReject in CSV"
            );
            for &class in &["InternalBugSentinel", "RealImpl"] {
                assert_eq!(
                    observed.get(class).copied().unwrap_or_default(),
                    documented.get(class).copied().unwrap_or_default(),
                    "{bucket} current {class} count should match the bucket doc"
                );
            }
            continue;
        }

        for &class in EXPECTED_CLASSES {
            assert_eq!(
                observed.get(class).copied().unwrap_or_default(),
                documented.get(class).copied().unwrap_or_default(),
                "{bucket} {class} count drift between inventory and bucket doc"
            );
        }
    }
}

#[test]
fn stable_id_matching_preserves_remaining_ids_after_simulated_deletion() {
    let initial = vec![
        test_snapshot_entry("UMB-0001", 10, "deleted row"),
        test_snapshot_entry("UMB-0002", 20, "kept row a"),
        test_snapshot_entry("UMB-0003", 30, "kept row b"),
    ];
    let retired = vec![test_retired_entry(&initial[0])];
    let generated = vec![
        test_inventory_entry(20, "kept row a"),
        test_inventory_entry(30, "kept row b"),
    ];

    let matched = match_stable_ids(generated, &initial, &retired);

    assert_eq!(
        matched
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        ["UMB-0002", "UMB-0003"]
    );
    validate_active_retired_partition(&matched, &initial, &retired);
}

#[test]
fn stable_id_matching_pairs_line_drift_by_source_order() {
    let initial = vec![
        test_snapshot_entry("UMB-0001", 10, "same group"),
        test_snapshot_entry("UMB-0002", 20, "same group"),
    ];
    let generated = vec![
        test_inventory_entry(12, "same group"),
        test_inventory_entry(25, "same group"),
    ];

    let matched = match_stable_ids(generated, &initial, &[]);

    assert_eq!(
        matched
            .iter()
            .map(|entry| (entry.id.as_str(), entry.line))
            .collect::<Vec<_>>(),
        [("UMB-0001", 12), ("UMB-0002", 25)]
    );
}

#[test]
#[should_panic(expected = "ambiguous stable ID match")]
fn stable_id_matching_rejects_ambiguous_group_count() {
    let initial = vec![test_snapshot_entry("UMB-0001", 10, "same group")];
    let generated = vec![
        test_inventory_entry(12, "same group"),
        test_inventory_entry(25, "same group"),
    ];

    let _ = match_stable_ids(generated, &initial, &[]);
}

#[test]
fn stable_id_matching_diff_mode_reports_unretired_deletion_without_panicking() {
    let initial = vec![
        test_snapshot_entry("UMB-0001", 10, "deleted row"),
        test_snapshot_entry("UMB-0002", 20, "kept row"),
    ];
    let generated = vec![test_inventory_entry(20, "kept row")];

    let matched = match_stable_ids_for_diff(generated, &initial, &[]);

    assert_eq!(
        matched
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        ["UMB-0002"]
    );
}

#[test]
fn stable_id_matching_diff_mode_reports_unmatched_addition_with_new_id() {
    let initial = vec![test_snapshot_entry("UMB-0001", 10, "kept row")];
    let generated = vec![
        test_inventory_entry(10, "kept row"),
        test_inventory_entry(20, "new row"),
    ];

    let matched = match_stable_ids_for_diff(generated, &initial, &[]);

    assert_eq!(
        matched
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        ["UMB-0001", "NEW-0001"]
    );
}

#[cfg(test)]
fn test_inventory_entry(line: usize, kind: &str) -> InventoryEntry {
    InventoryEntry {
        id: String::new(),
        file: "crates/scoopc/src/llvm/codegen/test.rs".to_string(),
        line,
        kind: kind.to_string(),
        route: "Helper",
        surface: "rvalue",
        bucket: "B-10",
        expected_class: "RealImpl",
        spec_anchor: "docs/spec/language_spec-part4.md#3-调用效果",
        upstream_gate: "test gate",
        existing_fixture: "",
        notes: "bucket_rule=effect-callable-adapter".to_string(),
        kind_source: KindSource::Literal,
    }
}

#[cfg(test)]
fn test_snapshot_entry(id: &str, line: usize, kind: &str) -> InventorySnapshotEntry {
    InventorySnapshotEntry {
        id: id.to_string(),
        file: "crates/scoopc/src/llvm/codegen/test.rs".to_string(),
        line,
        kind: kind.to_string(),
        surface: "rvalue".to_string(),
        bucket: "B-10".to_string(),
        expected_class: "RealImpl".to_string(),
    }
}

#[cfg(test)]
fn test_retired_entry(initial: &InventorySnapshotEntry) -> RetiredEntry {
    RetiredEntry {
        id: initial.id.clone(),
        bucket: initial.bucket.clone(),
        expected_class: initial.expected_class.clone(),
        file: initial.file.clone(),
        old_line: initial.line,
        kind: initial.kind.clone(),
        retired_by: "test".to_string(),
        retired_reason: "simulated deletion".to_string(),
        retired_at_notes: String::new(),
    }
}

#[cfg(test)]
pub(crate) fn inventory_entries() -> Vec<InventoryEntry> {
    let generated = generated_inventory_entries();
    let initial = read_initial_inventory_snapshot();
    let retired = read_retired_ledger();
    inherit_stable_ids(generated, &initial, &retired)
}

pub(crate) fn inventory_entries_for_diff() -> Vec<InventoryEntry> {
    let generated = generated_inventory_entries();
    let initial = read_initial_inventory_snapshot();
    let retired = read_retired_ledger();
    validate_initial_snapshot(&initial);
    validate_retired_ledger(&retired, &initial);
    match_stable_ids_for_diff(generated, &initial, &retired)
}

fn generated_inventory_entries() -> Vec<InventoryEntry> {
    let mut source_entries = scan_source_entries();
    source_entries.sort_by(|left, right| {
        (&left.file, left.line, left.column).cmp(&(&right.file, right.line, right.column))
    });

    source_entries
        .into_iter()
        .map(|source| {
            let classification = classify(&source);
            InventoryEntry {
                id: String::new(),
                file: source.file.clone(),
                line: source.line,
                kind: source.kind.clone(),
                route: route_for_path(&source.file),
                surface: surface_for(&source),
                bucket: classification.bucket,
                expected_class: classification.expected_class,
                spec_anchor: classification.spec_anchor,
                upstream_gate: classification.upstream_gate,
                existing_fixture: "",
                notes: classification.notes,
                kind_source: source.kind_source,
            }
        })
        .collect()
}

type ExactKey = (String, usize, String);
type StableKey = (String, String, String, String, String);

#[cfg(test)]
fn inherit_stable_ids(
    generated: Vec<InventoryEntry>,
    initial: &[InventorySnapshotEntry],
    retired: &[RetiredEntry],
) -> Vec<InventoryEntry> {
    validate_initial_snapshot(initial);
    validate_retired_ledger(retired, initial);
    match_stable_ids(generated, initial, retired)
}

#[cfg(test)]
fn match_stable_ids(
    generated: Vec<InventoryEntry>,
    initial: &[InventorySnapshotEntry],
    retired: &[RetiredEntry],
) -> Vec<InventoryEntry> {
    match_stable_ids_with_mode(generated, initial, retired, StableIdMode::Strict)
}

fn match_stable_ids_for_diff(
    generated: Vec<InventoryEntry>,
    initial: &[InventorySnapshotEntry],
    retired: &[RetiredEntry],
) -> Vec<InventoryEntry> {
    match_stable_ids_with_mode(generated, initial, retired, StableIdMode::Diff)
}

fn match_stable_ids_with_mode(
    mut generated: Vec<InventoryEntry>,
    initial: &[InventorySnapshotEntry],
    retired: &[RetiredEntry],
    mode: StableIdMode,
) -> Vec<InventoryEntry> {
    let retired_ids = retired
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<BTreeSet<_>>();
    let active_initial = initial
        .iter()
        .filter(|entry| !retired_ids.contains(entry.id.as_str()))
        .collect::<Vec<_>>();
    let mut assignments = vec![None; generated.len()];
    let mut remaining_generated = (0..generated.len()).collect::<BTreeSet<_>>();
    let mut remaining_initial = (0..active_initial.len()).collect::<BTreeSet<_>>();

    assign_unique_exact_matches(
        &generated,
        &active_initial,
        &mut assignments,
        &mut remaining_generated,
        &mut remaining_initial,
    );
    assign_unique_stable_key_matches(
        &generated,
        &active_initial,
        &mut assignments,
        &mut remaining_generated,
        &mut remaining_initial,
    );
    assign_ordered_stable_key_matches(
        &generated,
        &active_initial,
        &mut assignments,
        &mut remaining_generated,
        &mut remaining_initial,
        mode,
    );

    let mut next_new_id = 1usize;
    for (entry, id) in generated.iter_mut().zip(assignments) {
        entry.id = match (id, mode) {
            (Some(id), _) => id,
            (None, StableIdMode::Strict) => {
                panic!(
                    "failed to assign stable UMB id to {}",
                    source_entry_summary(entry)
                )
            }
            (None, StableIdMode::Diff) => {
                let id = format!("NEW-{next_new_id:04}");
                next_new_id += 1;
                id
            }
        };
    }
    if mode == StableIdMode::Strict {
        validate_active_retired_partition(&generated, initial, retired);
    }
    generated
}

fn assign_unique_exact_matches(
    generated: &[InventoryEntry],
    active_initial: &[&InventorySnapshotEntry],
    assignments: &mut [Option<String>],
    remaining_generated: &mut BTreeSet<usize>,
    remaining_initial: &mut BTreeSet<usize>,
) {
    let generated_by_exact = group_generated_by_exact(generated, remaining_generated);
    let initial_by_exact = group_initial_by_exact(active_initial, remaining_initial);
    let mut keys = BTreeSet::new();
    keys.extend(generated_by_exact.keys().cloned());
    keys.extend(initial_by_exact.keys().cloned());

    for key in keys {
        let generated_indices = generated_by_exact
            .get(&key)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let initial_indices = initial_by_exact.get(&key).map(Vec::as_slice).unwrap_or(&[]);
        if let ([generated_idx], [initial_idx]) = (generated_indices, initial_indices) {
            assign_stable_id(
                *generated_idx,
                *initial_idx,
                active_initial,
                assignments,
                remaining_generated,
                remaining_initial,
            );
        }
    }
}

fn assign_unique_stable_key_matches(
    generated: &[InventoryEntry],
    active_initial: &[&InventorySnapshotEntry],
    assignments: &mut [Option<String>],
    remaining_generated: &mut BTreeSet<usize>,
    remaining_initial: &mut BTreeSet<usize>,
) {
    let generated_by_key = group_generated_by_stable_key(generated, remaining_generated);
    let initial_by_key = group_initial_by_stable_key(active_initial, remaining_initial);
    let mut keys = BTreeSet::new();
    keys.extend(generated_by_key.keys().cloned());
    keys.extend(initial_by_key.keys().cloned());

    for key in keys {
        let generated_indices = generated_by_key.get(&key).map(Vec::as_slice).unwrap_or(&[]);
        let initial_indices = initial_by_key.get(&key).map(Vec::as_slice).unwrap_or(&[]);
        if let ([generated_idx], [initial_idx]) = (generated_indices, initial_indices) {
            assign_stable_id(
                *generated_idx,
                *initial_idx,
                active_initial,
                assignments,
                remaining_generated,
                remaining_initial,
            );
        }
    }
}

fn assign_ordered_stable_key_matches(
    generated: &[InventoryEntry],
    active_initial: &[&InventorySnapshotEntry],
    assignments: &mut [Option<String>],
    remaining_generated: &mut BTreeSet<usize>,
    remaining_initial: &mut BTreeSet<usize>,
    mode: StableIdMode,
) {
    let generated_by_key = group_generated_by_stable_key(generated, remaining_generated);
    let initial_by_key = group_initial_by_stable_key(active_initial, remaining_initial);
    let mut keys = BTreeSet::new();
    keys.extend(generated_by_key.keys().cloned());
    keys.extend(initial_by_key.keys().cloned());

    for key in keys {
        let generated_indices = generated_by_key.get(&key).cloned().unwrap_or_default();
        let initial_indices = initial_by_key.get(&key).cloned().unwrap_or_default();
        if generated_indices.is_empty() {
            if mode == StableIdMode::Diff {
                continue;
            }
            panic!(
                "active inventory is missing unretired initial rows for {}: {}",
                stable_key_label(&key),
                initial_indices
                    .iter()
                    .map(|idx| initial_entry_summary(active_initial[*idx]))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
        if mode == StableIdMode::Strict
            && (initial_indices.is_empty() || generated_indices.len() != initial_indices.len())
        {
            panic!(
                "ambiguous stable ID match for {}: generated [{}], initial [{}]",
                stable_key_label(&key),
                generated_indices
                    .iter()
                    .map(|idx| source_entry_summary(&generated[*idx]))
                    .collect::<Vec<_>>()
                    .join("; "),
                initial_indices
                    .iter()
                    .map(|idx| initial_entry_summary(active_initial[*idx]))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }

        let mut generated_sorted = generated_indices;
        generated_sorted.sort_by(|left, right| {
            generated[*left]
                .line
                .cmp(&generated[*right].line)
                .then_with(|| generated[*left].kind.cmp(&generated[*right].kind))
        });
        let mut initial_sorted = initial_indices;
        initial_sorted.sort_by(|left, right| {
            let left_entry = active_initial[*left];
            let right_entry = active_initial[*right];
            left_entry
                .line
                .cmp(&right_entry.line)
                .then_with(|| left_entry.id.cmp(&right_entry.id))
        });

        for (generated_idx, initial_idx) in generated_sorted.into_iter().zip(initial_sorted) {
            assign_stable_id(
                generated_idx,
                initial_idx,
                active_initial,
                assignments,
                remaining_generated,
                remaining_initial,
            );
        }
    }
}

fn assign_stable_id(
    generated_idx: usize,
    initial_idx: usize,
    active_initial: &[&InventorySnapshotEntry],
    assignments: &mut [Option<String>],
    remaining_generated: &mut BTreeSet<usize>,
    remaining_initial: &mut BTreeSet<usize>,
) {
    assert!(
        remaining_generated.remove(&generated_idx),
        "generated entry {generated_idx} was assigned more than once"
    );
    assert!(
        remaining_initial.remove(&initial_idx),
        "initial entry {initial_idx} was assigned more than once"
    );
    assignments[generated_idx] = Some(active_initial[initial_idx].id.clone());
}

fn group_generated_by_exact(
    generated: &[InventoryEntry],
    remaining: &BTreeSet<usize>,
) -> BTreeMap<ExactKey, Vec<usize>> {
    let mut groups = BTreeMap::new();
    for &idx in remaining {
        groups
            .entry(exact_key_for_entry(&generated[idx]))
            .or_insert_with(Vec::new)
            .push(idx);
    }
    groups
}

fn group_initial_by_exact(
    initial: &[&InventorySnapshotEntry],
    remaining: &BTreeSet<usize>,
) -> BTreeMap<ExactKey, Vec<usize>> {
    let mut groups = BTreeMap::new();
    for &idx in remaining {
        groups
            .entry(exact_key_for_snapshot(initial[idx]))
            .or_insert_with(Vec::new)
            .push(idx);
    }
    groups
}

fn group_generated_by_stable_key(
    generated: &[InventoryEntry],
    remaining: &BTreeSet<usize>,
) -> BTreeMap<StableKey, Vec<usize>> {
    let mut groups = BTreeMap::new();
    for &idx in remaining {
        groups
            .entry(stable_key_for_entry(&generated[idx]))
            .or_insert_with(Vec::new)
            .push(idx);
    }
    groups
}

fn group_initial_by_stable_key(
    initial: &[&InventorySnapshotEntry],
    remaining: &BTreeSet<usize>,
) -> BTreeMap<StableKey, Vec<usize>> {
    let mut groups = BTreeMap::new();
    for &idx in remaining {
        groups
            .entry(stable_key_for_snapshot(initial[idx]))
            .or_insert_with(Vec::new)
            .push(idx);
    }
    groups
}

fn exact_key_for_entry(entry: &InventoryEntry) -> ExactKey {
    (entry.file.clone(), entry.line, entry.kind.clone())
}

fn exact_key_for_snapshot(entry: &InventorySnapshotEntry) -> ExactKey {
    (entry.file.clone(), entry.line, entry.kind.clone())
}

fn stable_key_for_entry(entry: &InventoryEntry) -> StableKey {
    (
        entry.file.clone(),
        entry.kind.clone(),
        entry.bucket.to_string(),
        entry.expected_class.to_string(),
        entry.surface.to_string(),
    )
}

fn stable_key_for_snapshot(entry: &InventorySnapshotEntry) -> StableKey {
    (
        entry.file.clone(),
        entry.kind.clone(),
        entry.bucket.clone(),
        entry.expected_class.clone(),
        entry.surface.clone(),
    )
}

fn stable_key_label(key: &StableKey) -> String {
    format!(
        "file={}, kind=`{}`, bucket={}, expected_class={}, surface={}",
        key.0, key.1, key.2, key.3, key.4
    )
}

fn source_entry_summary(entry: &InventoryEntry) -> String {
    let id = if entry.id.is_empty() {
        "<unassigned>"
    } else {
        entry.id.as_str()
    };
    format!(
        "{} {}:{} {} {} `{}`",
        id, entry.file, entry.line, entry.bucket, entry.expected_class, entry.kind
    )
}

fn initial_entry_summary(entry: &InventorySnapshotEntry) -> String {
    format!(
        "{} {}:{} {} {} `{}`",
        entry.id, entry.file, entry.line, entry.bucket, entry.expected_class, entry.kind
    )
}

fn validate_initial_snapshot(initial: &[InventorySnapshotEntry]) {
    assert_eq!(
        initial.len(),
        INITIAL_ENTRY_COUNT,
        "{INITIAL_INVENTORY_PATH} must preserve the frozen {INITIAL_ENTRY_COUNT}-row UMB baseline"
    );

    let mut ids = BTreeSet::new();
    for (index, entry) in initial.iter().enumerate() {
        let expected_id = format!("UMB-{number:04}", number = index + 1);
        assert_eq!(
            entry.id,
            expected_id,
            "{INITIAL_INVENTORY_PATH} row {} must keep contiguous baseline ID {expected_id}",
            index + 2
        );
        assert!(
            ids.insert(entry.id.as_str()),
            "duplicate initial id: {}",
            entry.id
        );
        assert!(
            VALID_BUCKETS.contains(&entry.bucket.as_str()),
            "{} has invalid initial bucket {}",
            entry.id,
            entry.bucket
        );
        assert!(
            matches!(
                entry.expected_class.as_str(),
                "FrontendReject" | "InternalBugSentinel" | "RealImpl"
            ),
            "{} has invalid initial expected_class {}",
            entry.id,
            entry.expected_class
        );
    }
}

fn validate_retired_ledger(retired: &[RetiredEntry], initial: &[InventorySnapshotEntry]) {
    let initial_by_id = initial
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut ids = BTreeSet::new();
    for entry in retired {
        assert!(
            ids.insert(entry.id.as_str()),
            "duplicate retired id: {}",
            entry.id
        );
        let Some(initial_entry) = initial_by_id.get(entry.id.as_str()) else {
            panic!(
                "{} contains retired id {} that is not in {INITIAL_INVENTORY_PATH}",
                RETIRED_LEDGER_PATH, entry.id
            );
        };
        assert_eq!(
            entry.bucket, initial_entry.bucket,
            "{} retired bucket drift",
            entry.id
        );
        assert_eq!(
            entry.expected_class, initial_entry.expected_class,
            "{} retired expected_class drift",
            entry.id
        );
        assert_eq!(
            entry.file, initial_entry.file,
            "{} retired file drift",
            entry.id
        );
        assert_eq!(
            entry.old_line, initial_entry.line,
            "{} retired old_line drift",
            entry.id
        );
        assert_eq!(
            entry.kind, initial_entry.kind,
            "{} retired kind drift",
            entry.id
        );
        assert!(
            !entry.retired_by.trim().is_empty(),
            "{} retired_by must identify the retiring task or commit",
            entry.id
        );
        assert!(
            !entry.retired_reason.trim().is_empty(),
            "{} retired_reason must explain the production fix",
            entry.id
        );
        assert!(
            !entry.retired_at_notes.contains("TBD"),
            "{} retired_at_notes must not use TBD placeholders",
            entry.id
        );
    }
}

fn validate_active_retired_partition(
    active: &[InventoryEntry],
    initial: &[InventorySnapshotEntry],
    retired: &[RetiredEntry],
) {
    let initial_count = initial.len();
    assert_eq!(
        active.len() + retired.len(),
        initial_count,
        "UMB countdown mismatch: active {} + retired {} must equal initial {initial_count}",
        active.len(),
        retired.len()
    );

    let active_ids = active
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<BTreeSet<_>>();
    let retired_ids = retired
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<BTreeSet<_>>();
    let initial_ids = initial
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<BTreeSet<_>>();

    let overlap = active_ids
        .intersection(&retired_ids)
        .copied()
        .collect::<Vec<_>>();
    assert!(
        overlap.is_empty(),
        "active and retired UMB ids must be disjoint; overlap: {}",
        overlap.join(", ")
    );

    let union = active_ids
        .union(&retired_ids)
        .copied()
        .collect::<BTreeSet<_>>();
    let missing = initial_ids.difference(&union).copied().collect::<Vec<_>>();
    let extra = union.difference(&initial_ids).copied().collect::<Vec<_>>();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "active + retired UMB ids must equal {INITIAL_INVENTORY_PATH}; missing [{}], extra [{}]",
        summarize_initial_ids(&missing, initial),
        extra.join(", ")
    );
}

fn summarize_initial_ids(ids: &[&str], initial: &[InventorySnapshotEntry]) -> String {
    let initial_by_id = initial
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    ids.iter()
        .map(|id| {
            initial_by_id
                .get(*id)
                .map(|entry| initial_entry_summary(entry))
                .unwrap_or_else(|| (*id).to_string())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn read_initial_inventory_snapshot() -> Vec<InventorySnapshotEntry> {
    read_csv_records(INITIAL_INVENTORY_PATH, CSV_HEADER)
        .into_iter()
        .enumerate()
        .map(|(index, record)| {
            InventorySnapshotEntry::from_record(INITIAL_INVENTORY_PATH, index + 2, record)
        })
        .collect()
}

fn read_retired_ledger() -> Vec<RetiredEntry> {
    read_csv_records(RETIRED_LEDGER_PATH, RETIRED_LEDGER_HEADER)
        .into_iter()
        .enumerate()
        .map(|(index, record)| RetiredEntry::from_record(index + 2, record))
        .collect()
}

pub(crate) fn retired_entry_count() -> usize {
    let initial = read_initial_inventory_snapshot();
    validate_initial_snapshot(&initial);
    let retired = read_retired_ledger();
    validate_retired_ledger(&retired, &initial);
    retired.len()
}

#[allow(dead_code)]
pub(crate) fn retired_ids_for_bucket(bucket: &str) -> BTreeSet<String> {
    let initial = read_initial_inventory_snapshot();
    validate_initial_snapshot(&initial);
    let retired = read_retired_ledger();
    validate_retired_ledger(&retired, &initial);
    retired
        .into_iter()
        .filter(|entry| entry.bucket == bucket)
        .map(|entry| entry.id)
        .collect()
}

fn read_csv_records(path_fragment: &str, expected_header: &str) -> Vec<Vec<String>> {
    let path = repo_root().join(path_fragment);
    let csv = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let mut records = parse_csv_records(&csv)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()));
    assert!(!records.is_empty(), "{} is empty", path.display());

    let header = records.remove(0);
    let expected_header = expected_header
        .split(',')
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        header,
        expected_header,
        "{} header mismatch",
        path.display()
    );
    records
}

fn parse_csv_records(csv: &str) -> Result<Vec<Vec<String>>, String> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut chars = csv.chars().peekable();
    let mut in_quotes = false;
    let mut saw_any = false;

    while let Some(ch) = chars.next() {
        saw_any = true;
        if in_quotes {
            match ch {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => in_quotes = false,
                _ => field.push(ch),
            }
            continue;
        }

        match ch {
            '"' if field.is_empty() => in_quotes = true,
            ',' => finish_csv_field(&mut record, &mut field),
            '\n' => finish_csv_record(&mut records, &mut record, &mut field),
            '\r' if chars.peek() == Some(&'\n') => {
                chars.next();
                finish_csv_record(&mut records, &mut record, &mut field);
            }
            '\r' => finish_csv_record(&mut records, &mut record, &mut field),
            _ => field.push(ch),
        }
    }

    if in_quotes {
        return Err("unterminated quoted CSV field".to_string());
    }
    if saw_any && (!record.is_empty() || !field.is_empty()) {
        finish_csv_record(&mut records, &mut record, &mut field);
    }

    Ok(records)
}

fn finish_csv_field(record: &mut Vec<String>, field: &mut String) {
    record.push(std::mem::take(field));
}

fn finish_csv_record(records: &mut Vec<Vec<String>>, record: &mut Vec<String>, field: &mut String) {
    finish_csv_field(record, field);
    if record.len() != 1 || !record[0].is_empty() {
        records.push(std::mem::take(record));
    } else {
        record.clear();
    }
}

impl InventorySnapshotEntry {
    fn from_record(path_fragment: &str, line_number: usize, record: Vec<String>) -> Self {
        let expected_len = CSV_HEADER.split(',').count();
        assert_eq!(
            record.len(),
            expected_len,
            "{path_fragment}:{line_number} has {} fields; expected {expected_len}",
            record.len()
        );

        Self {
            id: record[0].clone(),
            file: record[1].clone(),
            line: parse_positive_usize(path_fragment, line_number, "line", &record[2]),
            kind: record[3].clone(),
            surface: record[5].clone(),
            bucket: record[6].clone(),
            expected_class: record[7].clone(),
        }
    }
}

impl RetiredEntry {
    fn from_record(line_number: usize, record: Vec<String>) -> Self {
        let expected_len = RETIRED_LEDGER_HEADER.split(',').count();
        assert_eq!(
            record.len(),
            expected_len,
            "{RETIRED_LEDGER_PATH}:{line_number} has {} fields; expected {expected_len}",
            record.len()
        );

        Self {
            id: record[0].clone(),
            bucket: record[1].clone(),
            expected_class: record[2].clone(),
            file: record[3].clone(),
            old_line: parse_positive_usize(
                RETIRED_LEDGER_PATH,
                line_number,
                "old_line",
                &record[4],
            ),
            kind: record[5].clone(),
            retired_by: record[6].clone(),
            retired_reason: record[7].clone(),
            retired_at_notes: record[8].clone(),
        }
    }
}

fn parse_positive_usize(
    path_fragment: &str,
    line_number: usize,
    field: &str,
    value: &str,
) -> usize {
    let parsed = value.parse::<usize>().unwrap_or_else(|err| {
        panic!("{path_fragment}:{line_number} has invalid {field} `{value}`: {err}")
    });
    assert!(
        parsed > 0,
        "{path_fragment}:{line_number} has non-positive {field} `{value}`"
    );
    parsed
}

fn scan_source_entries() -> Vec<SourceEntry> {
    let root = repo_root();
    let codegen_root = root.join(CODEGEN_ROOT);
    let mut files = Vec::new();
    collect_rs_files(&codegen_root, &mut files);
    files.sort();

    let mut entries = Vec::new();
    for path in files {
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let file = repo_relative_path(&root, &path);
        let mut cursor = 0usize;
        while let Some(offset) = content[cursor..].find("UnsupportedMainBody") {
            let start = cursor + offset;
            let after_name = start + "UnsupportedMainBody".len();
            let open_brace = skip_ascii_whitespace(&content, after_name);
            if !content[open_brace..].starts_with('{') {
                cursor = after_name;
                continue;
            }

            let close_brace = find_matching_brace(&content, open_brace).unwrap_or_else(|| {
                panic!(
                    "unclosed UnsupportedMainBody initializer in {file}:{}",
                    line_number(&content, start)
                )
            });
            let initializer = &content[open_brace..=close_brace];
            let (kind, kind_source) = extract_kind(initializer).unwrap_or_else(|| {
                panic!(
                    "missing kind field in UnsupportedMainBody initializer at {file}:{}",
                    line_number(&content, start)
                )
            });

            entries.push(SourceEntry {
                file: file.clone(),
                line: line_number(&content, start),
                column: column_number(&content, start),
                kind,
                kind_source,
            });
            cursor = close_brace + 1;
        }
    }

    entries
}

fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(dir).unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
    {
        let path = entry
            .unwrap_or_else(|err| panic!("failed to read entry in {}: {err}", dir.display()))
            .path();
        if path.is_dir() {
            collect_rs_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn classify(source: &SourceEntry) -> Classification {
    let lower = source.kind.to_ascii_lowercase();
    let bucket = bucket_for(&source.file, &lower);
    let mut notes = Vec::new();

    notes.push(format!("bucket_rule={}", bucket_name(bucket)));
    if source.kind_source == KindSource::Dynamic {
        notes.push("kind_field=dynamic-expression".to_string());
    }
    notes.extend(
        legacy_gap_ids_for(bucket, &lower)
            .into_iter()
            .map(|gap_id| format!("legacy_gap={gap_id}")),
    );
    if bucket == "B-36" && !contains_any(&lower, &["todo", "async", "generator", "yield"]) {
        notes.push("bucket_decision=spec-uncovered-or-placeholder-surface".to_string());
    }

    Classification {
        bucket,
        expected_class: expected_class_for(bucket, &lower),
        spec_anchor: spec_anchor_for(bucket),
        upstream_gate: upstream_gate_for(bucket),
        notes: notes.join("; "),
    }
}

fn bucket_for(path: &str, lower: &str) -> &'static str {
    if contains_any(
        lower,
        &[
            "builder has no",
            "insert block",
            "parent function",
            "current function",
            "entry block",
        ],
    ) {
        return "B-01";
    }

    if contains_any(
        lower,
        &["todo", "not supported yet", "async", "generator", "yield"],
    ) || matches!(lower, "expression" | "statement")
    {
        return "B-36";
    }

    if path.ends_with("intrinsics/atomic.rs")
        || contains_any(lower, &["atomicint", "atomicref", "atomic "])
    {
        return "B-26";
    }
    if path.ends_with("intrinsics/sync.rs")
        || contains_any(lower, &["sync.", "condvar", "mutex", "once."])
    {
        return "B-27";
    }
    if path.ends_with("intrinsics/thread.rs") || lower.contains("thread.") {
        return "B-28";
    }
    if contains_any(lower, &["stackmap", "statepoint"])
        && !contains_any(lower, &["spill slot", "frame slot", "gc ptr"])
    {
        return "B-30";
    }
    if path.ends_with("gc.rs")
        || contains_any(
            lower,
            &["gc.", "gc ", "handleget", "handlenew", "handledrop"],
        )
    {
        return "B-29";
    }
    if contains_any(
        lower,
        &[
            "equivalent_codegen",
            "typestore",
            "codegen type",
            "cg type",
            "contract codegen type",
        ],
    ) {
        return "B-09";
    }

    if contains_any(
        lower,
        &[
            "runtimeerror",
            "runtime error",
            "completion payload",
            "try",
            "catch",
            "finally",
            "unwind",
        ],
    ) {
        return "B-34";
    }
    if contains_any(
        lower,
        &[
            "transport",
            "composite",
            "array",
            "to string",
            "descriptor-backed",
            "replay block",
            "task payload",
            "task result",
        ],
    ) {
        return "B-13";
    }
    if contains_any(
        lower,
        &[
            "effect-typed",
            "effect outcome",
            "plain adapter",
            "adapter carrier",
            "perform",
            "continuation",
            "resume",
            "outward",
            "handler",
        ],
    ) || path.contains("/effect_lowered/")
    {
        return "B-10";
    }
    if contains_any(lower, &["closure env", "capture", "lambda", "closure "]) {
        return "B-12";
    }
    if contains_any(
        lower,
        &[
            "runtime type",
            "rtti",
            "platform",
            "getplatform",
            "type descriptor",
        ],
    ) {
        return "B-25";
    }
    if contains_any(
        lower,
        &[
            "reflection",
            "descof",
            "kindof",
            "sizeof",
            "comptime",
            "splice",
            "annotation",
        ],
    ) {
        return "B-24";
    }
    if contains_any(
        lower,
        &["as?", " cast", "type check", "type-test", "smart cast"],
    ) || path.ends_with("mir_body/cast.rs")
    {
        return "B-14";
    }
    if lower.contains("when") {
        return "B-15";
    }
    if path.ends_with("mir_body/const_pat.rs")
        || contains_any(lower, &["pattern", "variant pattern", "tuple pattern"])
    {
        return "B-07";
    }
    if contains_any(
        lower,
        &[
            "break outside",
            "continue outside",
            "return outside",
            "outside loop",
            "outside function",
        ],
    ) {
        return "B-16";
    }
    if contains_any(
        lower,
        &["pure statement", "plain statement", "effectful direct call"],
    ) || path.ends_with("stmt.rs")
    {
        return "B-11";
    }
    if contains_any(
        lower,
        &[
            "coerce",
            "operator",
            "equality",
            "comparison",
            "arithmetic",
            "numeric",
            "scalar",
            "bool operator",
            "u64 word",
        ],
    ) || path.ends_with("main/coerce.rs")
        || path.ends_with("main/expr_op.rs")
    {
        return "B-17";
    }
    if contains_any(
        lower,
        &[
            "float.",
            "int.",
            "char.",
            "bool.",
            "string.",
            "bytelength",
            "hash receiver",
        ],
    ) {
        return "B-31";
    }
    if contains_any(
        lower,
        &[
            "print",
            "println",
            "panic",
            "__scoop_print_string",
            "sysroot panic",
        ],
    ) {
        return "B-32";
    }
    if contains_any(
        lower,
        &["top-level funptr", "extern global", "funptr value metadata"],
    ) {
        return "B-33";
    }
    if contains_any(
        lower,
        &[
            "funptr",
            "named intrinsic",
            "runtime intrinsic",
            "unsafe",
            "uintptr",
        ],
    ) || path.ends_with("intrinsics/named.rs")
        || path.ends_with("intrinsics/builtin.rs")
    {
        return "B-30";
    }
    if contains_any(
        lower,
        &[
            "nogc",
            "primitive boundary",
            "gc ptr",
            "spill slot",
            "frame slot",
        ],
    ) {
        return "B-35";
    }
    if contains_any(
        lower,
        &["top-level", "object init", "object property", "global"],
    ) || path.ends_with("object_init.rs")
        || path.ends_with("main/globals.rs")
    {
        return "B-19";
    }
    if contains_any(
        lower,
        &["class ctor", "class field", "property", "constructor"],
    ) || path.ends_with("class_ctor.rs")
    {
        return "B-20";
    }
    if contains_any(
        lower,
        &[
            "struct field",
            "unknown struct field",
            "field index",
            "missing field",
        ],
    ) {
        return "B-21";
    }
    if contains_any(
        lower,
        &[
            "option",
            "niche",
            "enum layout",
            "enum payload",
            "enum variant",
            "variant arity",
        ],
    ) || path.ends_with("enum_lowering.rs")
    {
        return "B-22";
    }
    if contains_any(
        lower,
        &[
            "struct literal",
            "tuple",
            "aggregate",
            "literal schema",
            "payload schema",
        ],
    ) || path.ends_with("mir_body/aggregates.rs")
    {
        return "B-06";
    }
    if contains_any(
        lower,
        &[
            "member store",
            "assignment",
            "immutable",
            "not writable",
            "store value",
        ],
    ) {
        return "B-08";
    }
    if contains_any(
        lower,
        &[
            "member access",
            "member target",
            "receiver",
            "field value",
            "field load",
        ],
    ) || path.ends_with("mir_body/member.rs")
    {
        return "B-23";
    }
    if contains_any(
        lower,
        &[
            "call arity",
            "call callee",
            "callee type",
            "call arg",
            "vtable call",
            "itable call",
            "direct call",
            "invoke arity",
        ],
    ) || path.ends_with("mir_body/call.rs")
        || path.ends_with("mir_body/callable_lookup.rs")
        || path.ends_with("call/lowering.rs")
        || path.ends_with("ordinary_callee.rs")
    {
        return "B-03";
    }
    if contains_any(
        lower,
        &[
            "param type",
            "return type",
            "sret",
            "function signature",
            "param arity",
            "argument type",
        ],
    ) || path.ends_with("mir_body/args.rs")
        || path.ends_with("mir_body/value_args.rs")
        || path.ends_with("main/function.rs")
    {
        return "B-04";
    }
    if contains_any(
        lower,
        &[
            "pass mir cfg",
            "start block",
            "goto target",
            "then target",
            "else target",
            "dispatch",
            "terminator",
            "block target",
        ],
    ) || path.ends_with("mir_body/terminator.rs")
        || path.ends_with("mir_body/dispatch.rs")
        || path.ends_with("control_flow.rs")
    {
        return "B-05";
    }
    if contains_any(
        lower,
        &[
            "local type",
            "local value",
            "source type",
            "member receiver type",
            "field type drift",
            "type lookup",
        ],
    ) || path.ends_with("mir_body/types.rs")
        || path.ends_with("main/immut_value.rs")
    {
        return "B-02";
    }
    if contains_any(
        lower,
        &[
            "literal",
            "int value",
            "bool value",
            "float value",
            "char value",
            "string value",
        ],
    ) || path.ends_with("main/literal.rs")
        || path.ends_with("mir_body/string.rs")
    {
        return "B-18";
    }

    "B-36"
}

fn expected_class_for(bucket: &str, lower: &str) -> &'static str {
    match bucket {
        "B-01" => "InternalBugSentinel",
        "B-10" | "B-12" | "B-13" | "B-24" | "B-25" => "RealImpl",
        "B-15" | "B-16" | "B-36" => "FrontendReject",
        "B-08" if contains_any(lower, &["immutable", "not writable", "assignment"]) => {
            "FrontendReject"
        }
        "B-21" if contains_any(lower, &["unknown", "duplicate", "missing field"]) => {
            "FrontendReject"
        }
        _ => "InternalBugSentinel",
    }
}

fn spec_anchor_for(bucket: &str) -> &'static str {
    match bucket {
        "B-01" => "N/A:helper-invariant",
        "B-02" => {
            "docs/spec/language_spec-part2.md#1-类型总览;docs/spec/language_spec-part3.md#17-类型推断"
        }
        "B-03" => {
            "docs/spec/language_spec-part3.md#4-调用参数与重载;docs/spec/language_spec-part2.md#11-function-type"
        }
        "B-04" => {
            "docs/spec/language_spec-part3.md#5-函数声明;docs/spec/language_spec-part2.md#11-function-type"
        }
        "B-05" => "docs/spec/language_spec-part3.md#9-控制流",
        "B-06" => {
            "docs/spec/language_spec-part2.md#5-值类型;docs/spec/language_spec-part3.md#14-struct-literalclosure-与-do-消歧"
        }
        "B-07" => "docs/spec/language_spec-part3.md#10-when-与模式匹配",
        "B-08" => {
            "docs/spec/language_spec-part3.md#2-变量绑定与赋值;docs/spec/language_spec-part3.md#8-属性"
        }
        "B-09" => {
            "docs/spec/language_spec-part2.md#1-类型总览;docs/spec/language_spec-part2.md#9-泛型"
        }
        "B-10" => {
            "docs/spec/language_spec-part4.md#3-调用效果;docs/spec/language_spec-part4.md#12-required-effect-推断"
        }
        "B-11" => {
            "docs/spec/language_spec-part3.md#1-表达式与语句;docs/spec/language_spec-part4.md#3-调用效果"
        }
        "B-12" => {
            "docs/spec/language_spec-part3.md#6-lambda-与函数值;docs/spec/language_spec-part3.md#61-函数类型与-effect"
        }
        "B-13" => {
            "docs/spec/language_spec-part3.md#12-数组字面量;docs/spec/language_spec-part2.md#5-值类型"
        }
        "B-14" => "docs/spec/language_spec-part3.md#11-类型测试smart-cast-与-cast",
        "B-15" => "docs/spec/language_spec-part3.md#10-when-与模式匹配",
        "B-16" => "docs/spec/language_spec-part3.md#94-return--break--continue",
        "B-17" => {
            "docs/spec/language_spec-part3.md#3-运算符优先级;docs/spec/language_spec-part2.md#3-内建标量类型"
        }
        "B-18" => {
            "docs/spec/language_spec-part1.md#6-字面量;docs/spec/language_spec-part2.md#3-内建标量类型"
        }
        "B-19" => {
            "docs/spec/language_spec-part1.md#4-顶层声明;docs/spec/language_spec-part2.md#45-object-与-companion-object"
        }
        "B-20" => {
            "docs/spec/language_spec-part2.md#42-构造器与初始化;docs/spec/language_spec-part3.md#8-属性"
        }
        "B-21" => {
            "docs/spec/language_spec-part2.md#51-struct;docs/spec/language_spec-part3.md#14-struct-literalclosure-与-do-消歧"
        }
        "B-22" => {
            "docs/spec/language_spec-part2.md#52-enum;docs/spec/language_spec-part2.md#6-nullable"
        }
        "B-23" => {
            "docs/spec/language_spec-part3.md#8-属性;docs/spec/language_spec-part3.md#14-struct-literalclosure-与-do-消歧"
        }
        "B-24" => {
            "docs/spec/language_spec-part5.md#6-静态反射-intrinsic;docs/spec/language_spec-part5.md#1-编译期执行总览"
        }
        "B-25" => {
            "docs/spec/language_spec-part5.md#9-platform-introspection;docs/spec/language_spec-part5.md#10-runtime-type-info"
        }
        "B-26" => "docs/spec/language_spec-part6.md#15-internal-atomic-value-types",
        "B-27" => "docs/spec/language_spec-part6.md#18-与标准库的关系",
        "B-28" => "docs/spec/language_spec-part6.md#18-与标准库的关系",
        "B-29" => {
            "docs/spec/language_spec-part6.md#12-gc-pinning;docs/spec/language_spec-part6.md#13-stable-gc-handle"
        }
        "B-30" => {
            "docs/spec/language_spec-part6.md#6-uintptr-与指针整数转换;docs/spec/language_spec-part6.md#7-function-pointer"
        }
        "B-31" => {
            "docs/spec/language_spec-part2.md#3-内建标量类型;docs/spec/language_spec-part3.md#7-extension-函数"
        }
        "B-32" => {
            "docs/spec/language_spec-part1.md#8-标准库排除说明;docs/spec/language_spec-part5.md#18-intrinsic-与-sysroot-声明"
        }
        "B-33" => {
            "docs/spec/language_spec-part1.md#4-顶层声明;docs/spec/language_spec-part6.md#8-extern"
        }
        "B-34" => {
            "docs/spec/language_spec-part4.md#8-try--catch--finally;docs/spec/language_spec-part4.md#9-runtimeerror"
        }
        "B-35" => {
            "docs/spec/language_spec-part6.md#2-unsafe-context;docs/spec/language_spec-part6.md#4-nogc"
        }
        "B-36" => {
            "docs/spec/language_spec-part4.md#10-async--structured-concurrency-surface当前未定义;docs/spec/language_spec-part4.md#11-generator--yield-风格"
        }
        _ => unreachable!("unknown bucket {bucket}"),
    }
}

fn upstream_gate_for(bucket: &str) -> &'static str {
    match bucket {
        "B-01" => "codegen helper invariant: active LLVM insertion context",
        "B-02" => "MIR materialize: local/member source and codegen types are complete",
        "B-03" => "MIR strict verifier: callable ABI, arity, and callee form are fixed",
        "B-04" => "MIR materialize: function signatures, params, and returns are published",
        "B-05" => "MIR strict verifier: CFG blocks and terminator targets are valid",
        "B-06" => "MIR strict verifier: aggregate literal schema matches lowered type layout",
        "B-07" => "typecheck pattern gate: pattern subject and clause schema are valid",
        "B-08" => {
            "typecheck assignment/member-store gate: target writability and value type are valid"
        }
        "B-09" => "TypeStore equivalence gate: cross-store codegen type lookup is total",
        "B-10" => {
            "effect lowering contract: callable adapter and effect ABI routing are implemented"
        }
        "B-11" => "HIR/MIR boundary gate: pure/plain statements are routed before LLVM lowering",
        "B-12" => {
            "closure lowering contract: capture environment layout and function value ABI are published"
        }
        "B-13" => "transport lowering contract: composite and array metadata are descriptor-backed",
        "B-14" => "typecheck/runtime-cast gate: cast and type-test target metadata are supported",
        "B-15" => {
            "typecheck when gate: exhaustiveness, subject type, guards, and arm types are valid"
        }
        "B-16" => "parser/HIR control-flow gate: break/continue/return context is valid",
        "B-17" => "typecheck/operator gate: scalar coercion and operator operands are valid",
        "B-18" => "parser/typecheck literal gate: literal target type and backing value are valid",
        "B-19" => {
            "resolve/typecheck top-level gate: object/global initializer metadata is complete"
        }
        "B-20" => {
            "resolve/typecheck class gate: ctor, property, and field access contracts are fixed"
        }
        "B-21" => "typecheck struct gate: field names, duplicates, and schema are valid",
        "B-22" => {
            "layout/typecheck enum gate: variant, niche, and Option payload contracts are valid"
        }
        "B-23" => "typecheck member-access gate: receiver and target member are resolved",
        "B-24" => {
            "comptime/reflection contract: static metadata intrinsics have supported lowering"
        }
        "B-25" => "platform/RTTI contract: platform and runtime type metadata are available",
        "B-26" => "intrinsic resolver gate: atomic intrinsic arity, target, and ordering are valid",
        "B-27" => "intrinsic resolver gate: sync primitive calls match sysroot runtime contract",
        "B-28" => "intrinsic resolver gate: thread primitive calls match sysroot runtime contract",
        "B-29" => "intrinsic resolver gate: GC handle/pin calls observe token and root contracts",
        "B-30" => "unsafe/intrinsic gate: named, FunPtr, and pointer conversion calls are valid",
        "B-31" => "typecheck scalar-extension gate: receiver and return scalar types are valid",
        "B-32" => "sysroot bridge gate: print/panic arguments are materialized as supported values",
        "B-33" => "top-level FFI gate: extern global and FunPtr metadata are complete",
        "B-34" => {
            "effect/runtime-error gate: try/catch/finally and RuntimeError layout are coherent"
        }
        "B-35" => "unsafe/NoGC gate: low-level boundary, frame, and stackmap invariants hold",
        "B-36" => {
            "spec/frontend gate: undefined or placeholder language surfaces are rejected before codegen"
        }
        _ => unreachable!("unknown bucket {bucket}"),
    }
}

fn route_for_path(path: &str) -> &'static str {
    if path.contains("/effect_lowered/") {
        "EffectLoweredLlvm"
    } else if path.contains("/mir_body/")
        || path.contains("/main/")
        || path.ends_with("class_ctor.rs")
        || path.ends_with("control_flow.rs")
        || path.ends_with("expr.rs")
        || path.ends_with("object_init.rs")
        || path.ends_with("ordinary_callee.rs")
        || path.ends_with("stmt.rs")
    {
        "RawMirLlvm"
    } else {
        "Helper"
    }
}

fn surface_for(source: &SourceEntry) -> &'static str {
    let lower = source.kind.to_ascii_lowercase();
    if source.bucket_like_builder(&lower) {
        return "builder";
    }
    if source.file.contains("/intrinsics/")
        || contains_any(&lower, &["intrinsic", "atomic", "sync.", "thread.", "gc."])
    {
        return "intrinsic";
    }
    if source.file.contains("terminator")
        || source.file.ends_with("control_flow.rs")
        || contains_any(
            &lower,
            &[
                "goto",
                "target",
                "terminator",
                "cfg",
                "return",
                "break",
                "continue",
            ],
        )
    {
        return "terminator";
    }
    if source.file.ends_with("stmt.rs") || lower.contains("statement") {
        return "stmt";
    }
    if source.file.ends_with("ty.rs")
        || source.file.ends_with("layout.rs")
        || source.file.ends_with("types.rs")
        || contains_any(&lower, &[" type", "layout", "signature"])
    {
        return "type";
    }
    "rvalue"
}

impl SourceEntry {
    fn bucket_like_builder(&self, lower: &str) -> bool {
        contains_any(
            lower,
            &[
                "builder has no",
                "insert block",
                "parent function",
                "current function",
                "entry block",
            ],
        )
    }
}

pub(crate) fn validate_inventory_entries(entries: &[InventoryEntry]) {
    let mut ids = BTreeSet::new();
    let mut locations = BTreeSet::new();
    for entry in entries {
        assert!(
            ids.insert(entry.id.as_str()),
            "duplicate active inventory id: {}",
            source_entry_summary(entry)
        );
        assert!(
            locations.insert((entry.file.as_str(), entry.line)),
            "duplicate source location in active inventory: {}",
            source_entry_summary(entry)
        );
        assert!(
            VALID_BUCKETS.contains(&entry.bucket),
            "{} has invalid bucket {}",
            source_entry_summary(entry),
            entry.bucket
        );
        assert!(
            matches!(
                entry.expected_class,
                "FrontendReject" | "InternalBugSentinel" | "RealImpl"
            ),
            "{} has invalid expected_class {}",
            source_entry_summary(entry),
            entry.expected_class
        );
        assert!(
            !entry.kind.trim().is_empty(),
            "{} has an empty kind field",
            source_entry_summary(entry)
        );
        assert!(
            !entry.spec_anchor.trim().is_empty(),
            "{} has an empty spec_anchor",
            source_entry_summary(entry)
        );
        assert!(
            !entry.upstream_gate.trim().is_empty(),
            "{} has an empty upstream_gate",
            source_entry_summary(entry)
        );
        assert!(
            !entry.notes.contains("TBD")
                && entry.bucket != "TBD"
                && entry.expected_class != "TBD"
                && entry.spec_anchor != "TBD"
                && entry.upstream_gate != "TBD",
            "{} still contains a TBD field",
            source_entry_summary(entry)
        );
        if entry.spec_anchor == "N/A:helper-invariant" {
            assert_eq!(
                entry.expected_class,
                "InternalBugSentinel",
                "helper invariant {} must be an internal bug sentinel",
                source_entry_summary(entry)
            );
        }
    }

    if std::env::var_os("SCOOP_DEBUG_UMB_DYNAMIC").is_some() {
        for entry in entries
            .iter()
            .filter(|entry| entry.kind_source == KindSource::Dynamic)
        {
            println!("dynamic kind: {}:{} {}", entry.file, entry.line, entry.kind);
        }
    }
    let literal_count = entries
        .iter()
        .filter(|entry| entry.kind_source == KindSource::Literal)
        .count();
    let dynamic_count = entries
        .iter()
        .filter(|entry| entry.kind_source == KindSource::Dynamic)
        .count();
    assert_eq!(
        literal_count + dynamic_count,
        entries.len(),
        "kind-source counts should sum to current active entries: literal {literal_count}, dynamic {dynamic_count}, active {}",
        entries.len()
    );
}

pub(crate) fn render_csv(entries: &[InventoryEntry]) -> String {
    let mut output = String::new();
    output.push_str(CSV_HEADER);
    output.push('\n');
    for entry in entries {
        let fields = vec![
            entry.id.clone(),
            entry.file.clone(),
            entry.line.to_string(),
            entry.kind.clone(),
            entry.route.to_string(),
            entry.surface.to_string(),
            entry.bucket.to_string(),
            entry.expected_class.to_string(),
            entry.spec_anchor.to_string(),
            entry.upstream_gate.to_string(),
            entry.existing_fixture.to_string(),
            entry.notes.clone(),
        ];
        output.push_str(
            &fields
                .iter()
                .map(|field| csv_escape(field))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
    }
    output
}

fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
fn bucket_distribution(entries: &[InventoryEntry]) -> String {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.bucket).or_insert(0usize) += 1;
    }
    VALID_BUCKETS
        .iter()
        .map(|bucket| {
            format!(
                "{bucket}: {}",
                counts.get(bucket).copied().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
fn bucket_counts(entries: &[InventoryEntry]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.bucket).or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
fn bucket_doc_entry_count(bucket: &str) -> usize {
    let content = read_bucket_doc(bucket);
    let prefix = "本 bucket entry 数：";
    let Some(line) = content.lines().find(|line| line.starts_with(prefix)) else {
        panic!("audit/UMB_categories/{bucket}.md is missing `{prefix}`");
    };
    line.trim_start_matches(prefix)
        .trim()
        .trim_end_matches("  ")
        .parse::<usize>()
        .unwrap_or_else(|err| panic!("failed to parse {bucket} entry count from `{line}`: {err}"))
}

#[cfg(test)]
fn bucket_doc_class_counts(bucket: &str) -> BTreeMap<String, usize> {
    let content = read_bucket_doc(bucket);
    let mut in_section = false;
    let mut counts = BTreeMap::new();

    for line in content.lines() {
        if line == "## Expected Post-Fix Class" {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("## ") {
            break;
        }
        if !in_section || !line.starts_with("| `") {
            continue;
        }

        let cells = line
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if cells.len() < 2 {
            continue;
        }
        let class = cells[0].trim_matches('`').to_string();
        let count = cells[1].parse::<usize>().unwrap_or_else(|err| {
            panic!("failed to parse {bucket} class count from `{line}`: {err}")
        });
        counts.insert(class, count);
    }

    for class in EXPECTED_CLASSES.iter().copied().chain(["D-pending"]) {
        assert!(
            counts.contains_key(class),
            "{bucket} Expected Post-Fix Class table is missing `{class}`"
        );
    }
    counts
}

#[cfg(test)]
fn read_bucket_doc(bucket: &str) -> String {
    let path = repo_root().join(format!("audit/UMB_categories/{bucket}.md"));
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

#[cfg(test)]
fn literal_kind_counts<'a>(kinds: impl Iterator<Item = &'a str>) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for kind in kinds {
        *counts.entry(kind.to_string()).or_insert(0) += 1;
    }
    counts
}

pub(crate) fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn repo_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or_else(|err| panic!("{} is not under {}: {err}", path.display(), root.display()))
        .to_string_lossy()
        .replace('\\', "/")
}

fn skip_ascii_whitespace(content: &str, mut index: usize) -> usize {
    while index < content.len() && content.as_bytes()[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn line_number(content: &str, index: usize) -> usize {
    content[..index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn column_number(content: &str, index: usize) -> usize {
    content[..index]
        .rfind('\n')
        .map_or(index + 1, |line_start| index - line_start)
}

fn find_matching_brace(content: &str, open_brace: usize) -> Option<usize> {
    let bytes = content.as_bytes();
    let mut index = open_brace;
    let mut depth = 0usize;
    let mut state = ScanState::Normal;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];
        match state {
            ScanState::Normal => match byte {
                b'"' => state = ScanState::String,
                b'\'' => state = ScanState::Char,
                b'/' if bytes.get(index + 1) == Some(&b'/') => {
                    state = ScanState::LineComment;
                    index += 1;
                }
                b'/' if bytes.get(index + 1) == Some(&b'*') => {
                    state = ScanState::BlockComment;
                    index += 1;
                }
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            },
            ScanState::String => {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    state = ScanState::Normal;
                }
            }
            ScanState::Char => {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'\'' {
                    state = ScanState::Normal;
                }
            }
            ScanState::LineComment => {
                if byte == b'\n' {
                    state = ScanState::Normal;
                }
            }
            ScanState::BlockComment => {
                if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    state = ScanState::Normal;
                    index += 1;
                }
            }
        }
        index += 1;
    }

    None
}

fn extract_kind(initializer: &str) -> Option<(String, KindSource)> {
    let mut cursor = 0usize;
    while let Some(relative) = initializer[cursor..].find("kind") {
        let position = cursor + relative;
        let before = position
            .checked_sub(1)
            .and_then(|index| initializer.as_bytes().get(index));
        let after = initializer.as_bytes().get(position + "kind".len());
        if before.is_some_and(|byte| is_identifier_byte(*byte))
            || after.is_some_and(|byte| is_identifier_byte(*byte))
        {
            cursor = position + "kind".len();
            continue;
        }

        let after_kind = skip_ascii_whitespace(initializer, position + "kind".len());
        if initializer.as_bytes().get(after_kind) == Some(&b':') {
            let value_start = skip_ascii_whitespace(initializer, after_kind + 1);
            if initializer.as_bytes().get(value_start) == Some(&b'"') {
                return parse_string_literal(initializer, value_start)
                    .map(|literal| (literal, KindSource::Literal));
            }
            return Some((
                format!(
                    "DYNAMIC:{}",
                    read_dynamic_expression(initializer, value_start)
                ),
                KindSource::Dynamic,
            ));
        }

        if matches!(
            initializer.as_bytes().get(after_kind),
            Some(b',') | Some(b'}')
        ) {
            return Some(("DYNAMIC:kind".to_string(), KindSource::Dynamic));
        }

        cursor = position + "kind".len();
    }
    None
}

fn parse_string_literal(content: &str, quote_start: usize) -> Option<String> {
    let bytes = content.as_bytes();
    let mut index = quote_start + 1;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            return Some(content[quote_start + 1..index].to_string());
        }
        index += 1;
    }
    None
}

fn read_dynamic_expression(content: &str, start: usize) -> String {
    let bytes = content.as_bytes();
    let mut index = start;
    let mut depth = 0usize;
    let mut state = ScanState::Normal;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];
        match state {
            ScanState::Normal => match byte {
                b'"' => state = ScanState::String,
                b'\'' => state = ScanState::Char,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                b',' if depth == 0 => break,
                _ => {}
            },
            ScanState::String => {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    state = ScanState::Normal;
                }
            }
            ScanState::Char => {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'\'' {
                    state = ScanState::Normal;
                }
            }
            ScanState::LineComment | ScanState::BlockComment => unreachable!(),
        }
        index += 1;
    }

    content[start..index]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone, Copy)]
enum ScanState {
    Normal,
    String,
    Char,
    LineComment,
    BlockComment,
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn bucket_name(bucket: &str) -> &'static str {
    match bucket {
        "B-01" => "builder-invariant",
        "B-02" => "mir-local-type",
        "B-03" => "call-abi",
        "B-04" => "function-signature",
        "B-05" => "mir-cfg",
        "B-06" => "literal-schema",
        "B-07" => "pattern-schema",
        "B-08" => "member-store",
        "B-09" => "type-equivalence",
        "B-10" => "effect-callable-adapter",
        "B-11" => "pure-boundary",
        "B-12" => "closure-capture",
        "B-13" => "composite-transport",
        "B-14" => "cast-typecheck",
        "B-15" => "when-pattern",
        "B-16" => "control-flow-context",
        "B-17" => "coercion-scalar",
        "B-18" => "literals-strings",
        "B-19" => "top-level-object-extern",
        "B-20" => "class-property-field",
        "B-21" => "struct-fields",
        "B-22" => "enum-niche-option",
        "B-23" => "member-access",
        "B-24" => "reflection-comptime",
        "B-25" => "platform-rtti",
        "B-26" => "atomic-intrinsics",
        "B-27" => "sync-intrinsics",
        "B-28" => "thread-intrinsics",
        "B-29" => "gc-intrinsics",
        "B-30" => "named-unsafe-funptr",
        "B-31" => "scalar-methods",
        "B-32" => "print-panic-sysroot",
        "B-33" => "extern-global-funptr",
        "B-34" => "runtime-error-try",
        "B-35" => "unsafe-nogc-boundary",
        "B-36" => "spec-uncovered",
        _ => unreachable!("unknown bucket {bucket}"),
    }
}

const LEGACY_GAP_BUCKETS: &[(&str, &str)] = &[
    ("PIPELINE_GAPS §2.3", "B-36"),
    ("PIPELINE_GAPS §3.1", "B-10"),
    ("PIPELINE_GAPS §3.2", "B-10"),
    ("PIPELINE_GAPS §3.3", "B-10"),
    ("PIPELINE_GAPS §3.5", "B-14"),
    ("PIPELINE_GAPS §3.6", "B-05"),
    ("PIPELINE_GAPS §3.8", "B-15"),
    ("PIPELINE_GAPS §3.10", "B-03"),
    ("PIPELINE_GAPS §3.11", "B-12"),
    ("PIPELINE_GAPS §3.12", "B-10"),
    ("PIPELINE_GAPS §3.13", "B-08"),
    ("PIPELINE_GAPS §4.1", "B-13"),
    ("PIPELINE_GAPS §4.5", "B-13"),
    ("PIPELINE_GAPS §5.1", "B-10"),
    ("PIPELINE_GAPS §5.3", "B-34"),
    ("PIPELINE_GAPS §5.4", "B-11"),
    ("PIPELINE_GAPS §7.2", "B-14"),
    ("PIPELINE_GAPS §7.6", "B-29"),
];

fn legacy_gap_ids_for(bucket: &str, lower: &str) -> Vec<&'static str> {
    let mut gap_ids = Vec::new();
    match bucket {
        "B-36" if lower.contains("todo") || matches!(lower, "expression" | "statement") => {
            gap_ids.push("PIPELINE_GAPS §2.3")
        }
        "B-10" => {
            if contains_any(lower, &["effect-control", "terminator"]) {
                gap_ids.push("PIPELINE_GAPS §3.1");
            }
            if lower.contains("perform route") || lower == "perform" {
                gap_ids.push("PIPELINE_GAPS §3.2");
            }
            if lower.contains("performresult") || lower.contains("perform result") {
                gap_ids.push("PIPELINE_GAPS §3.3");
            }
            if contains_any(lower, &["effect-typed callable adapter", "plain adapter"]) {
                gap_ids.push("PIPELINE_GAPS §3.12");
            }
            if lower.contains("outward") {
                gap_ids.push("PIPELINE_GAPS §5.1");
            }
        }
        "B-14" => {
            if contains_any(lower, &["runtime cast", "typecheck", "type check", "as?"]) {
                gap_ids.push("PIPELINE_GAPS §3.5");
            }
            if lower.contains("function-type") || lower.contains("function type runtime") {
                gap_ids.push("PIPELINE_GAPS §7.2");
            }
        }
        "B-05" if contains_any(lower, &["dispatch", "resume", "handoff"]) => {
            gap_ids.push("PIPELINE_GAPS §3.6")
        }
        "B-15" if lower.contains("when") || lower.contains("pattern") => {
            gap_ids.push("PIPELINE_GAPS §3.8")
        }
        "B-20" if lower.contains("class ctor") => gap_ids.push("PIPELINE_GAPS §3.9"),
        "B-03" if lower.contains("default") || lower.contains("call") => {
            gap_ids.push("PIPELINE_GAPS §3.10")
        }
        "B-12" if lower.contains("closure env") => gap_ids.push("PIPELINE_GAPS §3.11"),
        "B-08" if lower.contains("member store") => gap_ids.push("PIPELINE_GAPS §3.13"),
        "B-13" => {
            if lower.contains("erasure") || lower.contains("descriptor") {
                gap_ids.push("PIPELINE_GAPS §4.1");
            }
            if lower.contains("array") {
                gap_ids.push("PIPELINE_GAPS §4.5");
            }
        }
        "B-22" => {
            if lower.contains("oversized") {
                gap_ids.push("PIPELINE_GAPS §4.3");
            }
            if lower.contains("boxed enum") {
                gap_ids.push("PIPELINE_GAPS §4.4");
            }
        }
        "B-34" if lower.contains("unwind") || lower.contains("cleanup") => {
            gap_ids.push("PIPELINE_GAPS §5.3")
        }
        "B-11" if lower.contains("plain") || lower.contains("pure") => {
            gap_ids.push("PIPELINE_GAPS §5.4")
        }
        "B-29" if contains_any(lower, &["gc.", "handle", "pin"]) => {
            gap_ids.push("PIPELINE_GAPS §7.6")
        }
        _ => {}
    }

    for (gap_id, legacy_bucket) in LEGACY_GAP_BUCKETS {
        if *legacy_bucket == bucket && !gap_ids.contains(gap_id) {
            gap_ids.push(*gap_id);
        }
    }
    gap_ids
}

#[cfg(test)]
fn legacy_gap_ids_from_notes(notes: &str) -> impl Iterator<Item = &str> + '_ {
    notes
        .split(';')
        .filter_map(|part| part.trim().strip_prefix("legacy_gap="))
}
