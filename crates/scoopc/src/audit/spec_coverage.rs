//! Baseline tests for the UMB fixture set and spec coverage matrix.
//!
//! These tests intentionally scan repository data files as text. They lock the
//! doc-and-test governance artifacts together without entering production
//! codegen paths.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::umb_inventory::{self, EXPECTED_ENTRY_COUNT, VALID_BUCKETS};

const UMB_FIX_ROOT: &str = "tests/fixtures/umb_fix";
const FIXTURE_INDEX_PATH: &str = "tests/fixtures/umb_fix/_index.csv";
const SPEC_COVERAGE_MATRIX_PATH: &str = "audit/spec_coverage_matrix.md";
const SENTINEL_README_PATH: &str = "tests/fixtures/umb_fix/B-01-builder-invariant/_README.md";
const EXPECTED_FIXTURE_INDEX_HEADER: &str =
    "fixture_path,bucket,kind,spec_anchor,umb_ids,status,notes";
const HEADER_SCAN_LINES: usize = 32;
const FORBIDDEN_NEGATIVE_TERMS: &[&str] =
    &["后端", "backend", "LLVM", "codegen", "UnsupportedMainBody"];

#[test]
fn umb_fix_fixture_index_in_sync() {
    let entries = fixture_index_entries();
    let indexed_paths = entries
        .iter()
        .map(|entry| entry.fixture_path.clone())
        .collect::<BTreeSet<_>>();
    let actual_paths = actual_fixture_paths();

    assert!(
        set_difference(&actual_paths, &indexed_paths).is_empty(),
        "fixtures missing from {FIXTURE_INDEX_PATH}: {}",
        set_difference(&actual_paths, &indexed_paths).join(", ")
    );
    assert!(
        set_difference(&indexed_paths, &actual_paths).is_empty(),
        "{FIXTURE_INDEX_PATH} rows pointing at missing fixtures: {}",
        set_difference(&indexed_paths, &actual_paths).join(", ")
    );

    for entry in &entries {
        validate_fixture_index_entry(entry);
    }
}

#[test]
fn umb_fix_every_inventory_id_is_covered() {
    let inventory_entries = umb_inventory::inventory_entries();
    let inventory_ids = inventory_entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();
    let b01_ids = inventory_entries
        .iter()
        .filter(|entry| entry.bucket == "B-01")
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();

    let mut covered = BTreeSet::new();
    for entry in fixture_index_entries() {
        covered.extend(parse_umb_id_list(&entry.umb_ids, &entry.fixture_path));
    }
    let sentinel_ids = sentinel_coverage_ids();
    assert_eq!(
        sentinel_ids, b01_ids,
        "B-01 sentinel coverage must exactly match helper-invariant inventory ids"
    );
    covered.extend(sentinel_ids);

    assert_eq!(
        inventory_ids.len(),
        EXPECTED_ENTRY_COUNT,
        "inventory id set should match the frozen baseline"
    );
    assert!(
        set_difference(&inventory_ids, &covered).is_empty(),
        "inventory ids without fixture or sentinel coverage: {}",
        set_difference(&inventory_ids, &covered).join(", ")
    );
    assert!(
        set_difference(&covered, &inventory_ids).is_empty(),
        "fixture or sentinel coverage references unknown inventory ids: {}",
        set_difference(&covered, &inventory_ids).join(", ")
    );
}

#[test]
fn umb_fix_every_bucket_has_at_least_one_pos_and_one_neg() {
    let mut by_bucket_kind: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for entry in fixture_index_entries() {
        *by_bucket_kind
            .entry(entry.bucket)
            .or_default()
            .entry(entry.kind)
            .or_insert(0) += 1;
    }

    for &bucket in VALID_BUCKETS {
        if bucket == "B-01" {
            assert!(
                !sentinel_coverage_ids().is_empty(),
                "B-01 is sentinel-only and must retain a sentinel coverage record"
            );
            continue;
        }

        let kinds = by_bucket_kind
            .get(bucket)
            .unwrap_or_else(|| panic!("{bucket} has no rows in {FIXTURE_INDEX_PATH}"));
        assert!(
            kinds.get("positive").copied().unwrap_or_default() > 0,
            "{bucket} must have at least one positive umb_fix fixture"
        );
        assert!(
            kinds.get("negative").copied().unwrap_or_default() > 0,
            "{bucket} must have at least one negative umb_fix fixture"
        );
    }
}

#[test]
fn umb_fix_spec_coverage_matrix_in_sync() {
    let matrix_path = repo_path(SPEC_COVERAGE_MATRIX_PATH);
    let matrix = fs::read_to_string(&matrix_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", matrix_path.display()));
    assert!(
        !matrix.contains("(planned)"),
        "{SPEC_COVERAGE_MATRIX_PATH} still contains planned fixture references"
    );

    let indexed_paths = fixture_index_entries()
        .into_iter()
        .map(|entry| entry.fixture_path)
        .collect::<BTreeSet<_>>();
    for path in backticked_paths_with_prefix(&matrix, UMB_FIX_ROOT) {
        assert!(
            repo_path(&path).is_file(),
            "{SPEC_COVERAGE_MATRIX_PATH} references missing fixture `{path}`"
        );
        assert!(
            indexed_paths.contains(&path),
            "{SPEC_COVERAGE_MATRIX_PATH} references `{path}` but it is absent from {FIXTURE_INDEX_PATH}"
        );
    }

    for bucket_link in matrix_bucket_links(&matrix) {
        assert!(
            repo_path(&format!("audit/{bucket_link}")).is_file(),
            "{SPEC_COVERAGE_MATRIX_PATH} references missing bucket doc `{bucket_link}`"
        );
    }

    let mut missing = Vec::new();
    for entry in umb_inventory::inventory_entries()
        .into_iter()
        .filter(|entry| entry.spec_anchor != "N/A:helper-invariant")
    {
        for anchor in entry.spec_anchor.split(';') {
            if !matrix.contains(anchor) {
                missing.push(format!("{} {anchor}", entry.id));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "inventory spec anchors missing from {SPEC_COVERAGE_MATRIX_PATH}: {}",
        missing.join(", ")
    );
}

#[test]
fn umb_fix_no_forbidden_terms_in_neg_messages() {
    for entry in fixture_index_entries()
        .into_iter()
        .filter(|entry| entry.kind == "negative")
    {
        let headers = FixtureHeaders::from_fixture(&entry.fixture_path);
        let messages = headers.directive_values("EXPECT-ERROR");
        assert!(
            !messages.is_empty(),
            "{} is negative but has no EXPECT-ERROR header",
            entry.fixture_path
        );

        for message in messages {
            for term in FORBIDDEN_NEGATIVE_TERMS {
                assert!(
                    !contains_forbidden_term(message, term),
                    "{} EXPECT-ERROR contains forbidden term `{term}`: {message}",
                    entry.fixture_path
                );
            }
        }
    }
}

pub(crate) fn sentinel_coverage_ids() -> BTreeSet<String> {
    let value = sentinel_marker_value("SENTINEL-COVERS");
    parse_umb_id_list(&value, SENTINEL_README_PATH)
        .into_iter()
        .collect()
}

pub(crate) fn sentinel_marker_value(marker: &str) -> String {
    let path = repo_path(SENTINEL_README_PATH);
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let prefix = format!("- {marker}:");
    let Some(line) = content
        .lines()
        .find(|line| line.trim_start().starts_with(&prefix))
    else {
        panic!("{} is missing `{prefix}`", path.display());
    };
    line.trim_start()
        .trim_start_matches(&prefix)
        .trim()
        .to_string()
}

fn fixture_index_entries() -> Vec<FixtureIndexEntry> {
    let path = repo_path(FIXTURE_INDEX_PATH);
    let csv = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let mut records = parse_csv_records(&csv, FIXTURE_INDEX_PATH);
    assert!(!records.is_empty(), "{FIXTURE_INDEX_PATH} is empty");
    let header = records.remove(0).join(",");
    assert_eq!(
        header, EXPECTED_FIXTURE_INDEX_HEADER,
        "{FIXTURE_INDEX_PATH} header mismatch"
    );

    records
        .into_iter()
        .enumerate()
        .map(|(index, record)| FixtureIndexEntry::from_record(index + 2, record))
        .collect()
}

fn validate_fixture_index_entry(entry: &FixtureIndexEntry) {
    assert!(
        entry.fixture_path.starts_with(UMB_FIX_ROOT) && entry.fixture_path.ends_with(".scoop"),
        "{} is not an umb_fix .scoop path",
        entry.fixture_path
    );
    assert!(
        VALID_BUCKETS.contains(&entry.bucket.as_str()),
        "{} has invalid bucket `{}`",
        entry.fixture_path,
        entry.bucket
    );
    assert!(
        matches!(entry.kind.as_str(), "positive" | "negative"),
        "{} has invalid fixture kind `{}`",
        entry.fixture_path,
        entry.kind
    );
    assert!(
        !entry.spec_anchor.trim().is_empty(),
        "{} has an empty spec_anchor in {FIXTURE_INDEX_PATH}",
        entry.fixture_path
    );
    assert!(
        !entry.notes.trim().is_empty(),
        "{} has empty notes in {FIXTURE_INDEX_PATH}",
        entry.fixture_path
    );
    assert_valid_status(entry);

    let headers = FixtureHeaders::from_fixture(&entry.fixture_path);
    for key in ["EXPECT", "SPEC", "COVERS", "BUCKETS"] {
        headers.require_single(key, &entry.fixture_path);
    }
    if entry.kind == "negative" {
        for key in [
            "EXPECT-ERROR-CODE",
            "EXPECT-ERROR-AT",
            "EXPECT-ERROR",
            "REASON",
        ] {
            headers.require_single(key, &entry.fixture_path);
        }
    }

    assert_eq!(
        headers.require_single("SPEC", &entry.fixture_path),
        entry.spec_anchor,
        "{} SPEC header drifted from {FIXTURE_INDEX_PATH}",
        entry.fixture_path
    );
    assert_eq!(
        parse_umb_id_list(
            headers.require_single("COVERS", &entry.fixture_path),
            &entry.fixture_path
        ),
        parse_umb_id_list(&entry.umb_ids, &entry.fixture_path),
        "{} COVERS header drifted from {FIXTURE_INDEX_PATH}",
        entry.fixture_path
    );

    let header_buckets = parse_bucket_list(headers.require_single("BUCKETS", &entry.fixture_path));
    for bucket in header_buckets {
        assert!(
            VALID_BUCKETS.contains(&bucket.as_str()),
            "{} BUCKETS header contains invalid bucket `{bucket}`",
            entry.fixture_path
        );
    }

    let expected_ignore = entry.status.strip_prefix("ignore-until-fix:");
    assert_eq!(
        headers.ignore_until_fix.as_deref(),
        expected_ignore,
        "{} ignore status drifted between header and {FIXTURE_INDEX_PATH}",
        entry.fixture_path
    );
}

fn assert_valid_status(entry: &FixtureIndexEntry) {
    if entry.status == "active" {
        return;
    }
    let Some(bucket) = entry.status.strip_prefix("ignore-until-fix:") else {
        panic!(
            "{} has invalid status `{}` in {FIXTURE_INDEX_PATH}",
            entry.fixture_path, entry.status
        );
    };
    assert_eq!(
        bucket, entry.bucket,
        "{} ignore-until-fix status should name its primary bucket",
        entry.fixture_path
    );
}

fn actual_fixture_paths() -> BTreeSet<String> {
    let root = repo_path(UMB_FIX_ROOT);
    let mut paths = Vec::new();
    collect_scoop_files(&root, &mut paths);
    paths
        .into_iter()
        .map(|path| repo_relative_path(&path))
        .collect()
}

fn collect_scoop_files(dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(dir).unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
    {
        let path = entry
            .unwrap_or_else(|err| panic!("failed to read entry in {}: {err}", dir.display()))
            .path();
        if path.is_dir() {
            collect_scoop_files(&path, paths);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "scoop")
        {
            paths.push(path);
        }
    }
}

fn backticked_paths_with_prefix(content: &str, prefix: &str) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    let mut rest = content;
    while let Some(start) = rest.find('`') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('`') else {
            break;
        };
        let token = &rest[..end];
        if token.starts_with(prefix) && token.ends_with(".scoop") {
            paths.insert(token.to_string());
        }
        rest = &rest[end + 1..];
    }
    paths
}

fn matrix_bucket_links(content: &str) -> BTreeSet<String> {
    content
        .split(['(', ')', '[', ']', '`', '|', ' ', '\n'])
        .filter(|token| token.starts_with("UMB_categories/B-") && token.ends_with(".md"))
        .map(str::to_string)
        .collect()
}

fn parse_bucket_list(value: &str) -> BTreeSet<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|bucket| !bucket.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_umb_id_list(value: &str, context: &str) -> Vec<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "NONE" {
        return Vec::new();
    }

    let mut seen = BTreeSet::new();
    trimmed
        .split(',')
        .map(|part| part.trim().to_string())
        .inspect(|id| {
            assert!(is_umb_id(id), "{context} contains invalid UMB id `{id}`");
            assert!(seen.insert(id.clone()), "{context} repeats UMB id `{id}`");
        })
        .collect()
}

fn is_umb_id(value: &str) -> bool {
    value.len() == "UMB-0000".len()
        && value.starts_with("UMB-")
        && value[4..].chars().all(|ch| ch.is_ascii_digit())
}

fn contains_forbidden_term(message: &str, term: &str) -> bool {
    if term.is_ascii() {
        message
            .to_ascii_lowercase()
            .contains(&term.to_ascii_lowercase())
    } else {
        message.contains(term)
    }
}

fn parse_csv_records(csv: &str, context: &str) -> Vec<Vec<String>> {
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

    assert!(!in_quotes, "{context} has an unterminated quoted CSV field");
    if saw_any && (!record.is_empty() || !field.is_empty()) {
        finish_csv_record(&mut records, &mut record, &mut field);
    }
    records
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

fn set_difference(left: &BTreeSet<String>, right: &BTreeSet<String>) -> Vec<String> {
    left.difference(right).cloned().collect()
}

fn repo_path(path: &str) -> PathBuf {
    umb_inventory::repo_root().join(path)
}

fn repo_relative_path(path: &Path) -> String {
    path.strip_prefix(umb_inventory::repo_root())
        .unwrap_or_else(|err| panic!("{} is outside repo root: {err}", path.display()))
        .to_string_lossy()
        .replace('\\', "/")
}

#[derive(Debug)]
struct FixtureIndexEntry {
    fixture_path: String,
    bucket: String,
    kind: String,
    spec_anchor: String,
    umb_ids: String,
    status: String,
    notes: String,
}

impl FixtureIndexEntry {
    fn from_record(line_number: usize, record: Vec<String>) -> Self {
        assert_eq!(
            record.len(),
            EXPECTED_FIXTURE_INDEX_HEADER.split(',').count(),
            "{FIXTURE_INDEX_PATH}:{line_number} has the wrong field count"
        );
        Self {
            fixture_path: record[0].clone(),
            bucket: record[1].clone(),
            kind: record[2].clone(),
            spec_anchor: record[3].clone(),
            umb_ids: record[4].clone(),
            status: record[5].clone(),
            notes: record[6].clone(),
        }
    }
}

struct FixtureHeaders {
    directives: BTreeMap<String, Vec<String>>,
    ignore_until_fix: Option<String>,
}

impl FixtureHeaders {
    fn from_fixture(fixture_path: &str) -> Self {
        let path = repo_path(fixture_path);
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let mut directives: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut ignore_until_fix = None;

        for line in content.lines().take(HEADER_SCAN_LINES) {
            let Some(comment) = line.trim_start().strip_prefix("//") else {
                continue;
            };
            let Some((raw_key, raw_value)) = comment.trim().split_once(':') else {
                continue;
            };
            let key = raw_key.trim().to_ascii_uppercase();
            let value = raw_value.trim().to_string();
            if key == "IGNORE-UNTIL-FIX" {
                ignore_until_fix = Some(value);
            } else {
                directives.entry(key).or_default().push(value);
            }
        }

        Self {
            directives,
            ignore_until_fix,
        }
    }

    fn directive_values(&self, key: &str) -> &[String] {
        self.directives
            .get(&key.to_ascii_uppercase())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn require_single(&self, key: &str, context: &str) -> &str {
        let values = self.directive_values(key);
        assert_eq!(
            values.len(),
            1,
            "{context} must contain exactly one {key} header"
        );
        &values[0]
    }
}
