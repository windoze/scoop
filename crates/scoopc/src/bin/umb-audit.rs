//! Command-line inventory audit tool for `audit/UMB_inventory.csv`.
//!
//! This binary is intentionally separate from the production compiler entrypoint:
//! it reads source text and audit CSV data, then reports inventory slices, drift,
//! and aggregate counts for governance tasks.

#[path = "../audit/umb_inventory.rs"]
mod umb_inventory;

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::process;

const EXPECTED_CLASSES: &[&str] = &["FrontendReject", "InternalBugSentinel", "RealImpl"];
const FIELD_DRIFT_COLUMNS: &[&str] = &[
    "route",
    "surface",
    "bucket",
    "expected_class",
    "spec_anchor",
    "upstream_gate",
    "existing_fixture",
    "notes",
];

fn main() {
    match run(env::args().skip(1)) {
        Ok(()) => {}
        Err(CliError::Usage(message)) => {
            eprintln!("{message}");
            process::exit(2);
        }
        Err(CliError::Failure(message)) => {
            eprintln!("{message}");
            process::exit(1);
        }
        Err(CliError::Drift(report)) => {
            println!("{report}");
            process::exit(1);
        }
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<(), CliError> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        return Err(CliError::Usage(usage()));
    };

    match command.as_str() {
        "list" => command_list(args),
        "diff" => command_diff(),
        "stats" => command_stats(),
        "-h" | "--help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(CliError::Usage(format!(
            "unknown umb-audit command `{other}`\n\n{}",
            usage()
        ))),
    }
}

fn command_list(args: impl Iterator<Item = String>) -> Result<(), CliError> {
    let filters = ListFilters::parse(args)?;
    let entries = read_inventory_entries()?;

    println!("id\tfile\tline\tbucket\texpected_class\troute\tsurface\tkind");
    let mut count = 0usize;
    for entry in entries.iter().filter(|entry| filters.matches(entry)) {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            entry.id,
            entry.file,
            entry.line,
            entry.bucket,
            entry.expected_class,
            entry.route,
            entry.surface,
            sanitize_tsv(&entry.kind)
        );
        count += 1;
    }
    println!("entries\t{count}");

    Ok(())
}

fn command_diff() -> Result<(), CliError> {
    let path = inventory_path();
    let on_disk = fs::read_to_string(&path)
        .map_err(|err| CliError::Failure(format!("failed to read {}: {err}", path.display())))?;
    let generated_entries = umb_inventory::inventory_entries_for_diff();
    umb_inventory::validate_inventory_entries(&generated_entries);
    let generated = umb_inventory::render_csv(&generated_entries);

    if on_disk == generated {
        println!(
            "{} is in sync ({} entries)",
            umb_inventory::INVENTORY_PATH,
            generated_entries.len()
        );
        return Ok(());
    }

    let report = diff_report(&on_disk, &generated)?;
    Err(CliError::Drift(report))
}

fn command_stats() -> Result<(), CliError> {
    let entries = read_inventory_entries()?;
    let active_count = entries.len();
    let retired_count = umb_inventory::retired_entry_count();
    let initial_count = umb_inventory::INITIAL_ENTRY_COUNT;
    if active_count + retired_count != initial_count {
        return Err(CliError::Failure(format!(
            "UMB countdown mismatch: active {active_count} + retired {retired_count} must equal initial {initial_count}"
        )));
    }

    let mut by_bucket = BTreeMap::new();
    let mut by_class = BTreeMap::new();
    let mut by_file = BTreeMap::new();

    for entry in &entries {
        *by_bucket.entry(entry.bucket.as_str()).or_insert(0usize) += 1;
        *by_class
            .entry(entry.expected_class.as_str())
            .or_insert(0usize) += 1;
        *by_file.entry(entry.file.as_str()).or_insert(0usize) += 1;
    }

    let missing_spec_anchor = entries
        .iter()
        .filter(|entry| missing_governance_field(&entry.spec_anchor))
        .count();
    let missing_upstream_gate = entries
        .iter()
        .filter(|entry| missing_governance_field(&entry.upstream_gate))
        .count();

    println!("active_entries\t{active_count}");
    println!("retired_entries\t{retired_count}");
    println!("initial_entries\t{initial_count}");
    println!("missing_spec_anchor\t{missing_spec_anchor}");
    println!("missing_upstream_gate\t{missing_upstream_gate}");

    println!("\nby_bucket");
    for &bucket in umb_inventory::VALID_BUCKETS {
        println!("{bucket}\t{}", by_bucket.get(bucket).copied().unwrap_or(0));
    }

    println!("\nby_class");
    for &class in EXPECTED_CLASSES {
        println!("{class}\t{}", by_class.get(class).copied().unwrap_or(0));
    }

    println!("\nby_file");
    for (file, count) in by_file {
        println!("{file}\t{count}");
    }

    Ok(())
}

fn read_inventory_entries() -> Result<Vec<CsvEntry>, CliError> {
    let path = inventory_path();
    let csv = fs::read_to_string(&path)
        .map_err(|err| CliError::Failure(format!("failed to read {}: {err}", path.display())))?;
    parse_inventory_csv(&csv)
}

fn inventory_path() -> std::path::PathBuf {
    umb_inventory::repo_root().join(umb_inventory::INVENTORY_PATH)
}

fn diff_report(on_disk: &str, generated: &str) -> Result<String, CliError> {
    let on_disk_entries = parse_inventory_csv(on_disk)?;
    let generated_entries = parse_inventory_csv(generated)?;
    let on_disk_by_id = by_id(&on_disk_entries);
    let generated_by_id = by_id(&generated_entries);
    let on_disk_ids = on_disk_by_id.keys().copied().collect::<BTreeSet<_>>();
    let generated_ids = generated_by_id.keys().copied().collect::<BTreeSet<_>>();

    let mut sections = Vec::new();
    let added = generated_ids
        .difference(&on_disk_ids)
        .map(|id| entry_summary(generated_by_id[id]))
        .collect::<Vec<_>>();
    if !added.is_empty() {
        sections.push(format_section("新增", &added));
    }

    let deleted = on_disk_ids
        .difference(&generated_ids)
        .map(|id| entry_summary(on_disk_by_id[id]))
        .collect::<Vec<_>>();
    if !deleted.is_empty() {
        sections.push(format_section("删除", &deleted));
    }

    let mut line_drift = Vec::new();
    let mut kind_drift = Vec::new();
    let mut field_drift = Vec::new();
    for id in on_disk_ids.intersection(&generated_ids) {
        let old = on_disk_by_id[id];
        let new = generated_by_id[id];
        if old.file != new.file || old.line != new.line {
            line_drift.push(format!(
                "{id}: {}:{} -> {}:{}",
                old.file, old.line, new.file, new.line
            ));
        }
        if old.kind != new.kind {
            kind_drift.push(format!("{id}: `{}` -> `{}`", old.kind, new.kind));
        }

        let changed_fields = FIELD_DRIFT_COLUMNS
            .iter()
            .filter_map(|field| {
                let old_value = old.field(field);
                let new_value = new.field(field);
                (old_value != new_value).then(|| format!("{field}: `{old_value}` -> `{new_value}`"))
            })
            .collect::<Vec<_>>();
        if !changed_fields.is_empty() {
            field_drift.push(format!("{id}: {}", changed_fields.join("; ")));
        }
    }

    if !line_drift.is_empty() {
        sections.push(format_section("line drift", &line_drift));
    }
    if !kind_drift.is_empty() {
        sections.push(format_section("kind drift", &kind_drift));
    }
    if !field_drift.is_empty() {
        sections.push(format_section("field drift", &field_drift));
    }
    if sections.is_empty() {
        sections
            .push("raw CSV text drift: parsed rows match but serialized bytes differ".to_string());
    }

    Ok(format!(
        "{} is out of sync\n{}",
        umb_inventory::INVENTORY_PATH,
        sections.join("\n")
    ))
}

fn by_id(entries: &[CsvEntry]) -> BTreeMap<&str, &CsvEntry> {
    entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect()
}

fn format_section(title: &str, lines: &[String]) -> String {
    let mut output = format!("{title} ({})", lines.len());
    for line in lines {
        output.push('\n');
        output.push_str("  ");
        output.push_str(line);
    }
    output
}

fn entry_summary(entry: &CsvEntry) -> String {
    format!(
        "{} {}:{} {} {} `{}`",
        entry.id, entry.file, entry.line, entry.bucket, entry.expected_class, entry.kind
    )
}

fn parse_inventory_csv(csv: &str) -> Result<Vec<CsvEntry>, CliError> {
    let mut records = parse_csv_records(csv)?;
    if records.is_empty() {
        return Err(CliError::Failure(format!(
            "{} is empty",
            umb_inventory::INVENTORY_PATH
        )));
    }

    let header = records.remove(0);
    let expected_header = csv_header_fields();
    if header != expected_header {
        return Err(CliError::Failure(format!(
            "{} header mismatch: expected `{}`, got `{}`",
            umb_inventory::INVENTORY_PATH,
            umb_inventory::CSV_HEADER,
            header.join(",")
        )));
    }

    records
        .into_iter()
        .enumerate()
        .map(|(index, record)| CsvEntry::from_record(index + 2, record))
        .collect()
}

fn parse_csv_records(csv: &str) -> Result<Vec<Vec<String>>, CliError> {
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
        return Err(CliError::Failure(
            "unterminated quoted CSV field".to_string(),
        ));
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

fn csv_header_fields() -> Vec<String> {
    umb_inventory::CSV_HEADER
        .split(',')
        .map(str::to_string)
        .collect()
}

fn missing_governance_field(value: &str) -> bool {
    value.trim().is_empty() || value.trim() == "TBD"
}

fn validate_bucket(bucket: &str) -> Result<(), CliError> {
    if umb_inventory::VALID_BUCKETS.contains(&bucket) {
        Ok(())
    } else {
        Err(CliError::Usage(format!(
            "invalid bucket `{bucket}`; expected B-01 through B-36"
        )))
    }
}

fn validate_class(class: &str) -> Result<(), CliError> {
    if EXPECTED_CLASSES.contains(&class) {
        Ok(())
    } else {
        Err(CliError::Usage(format!(
            "invalid class `{class}`; expected FrontendReject, InternalBugSentinel, or RealImpl"
        )))
    }
}

fn sanitize_tsv(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if matches!(ch, '\t' | '\n' | '\r') {
                ' '
            } else {
                ch
            }
        })
        .collect()
}

fn usage() -> String {
    "usage:\n  umb-audit list [--bucket B-XX] [--file PATH] [--class CLASS]\n  umb-audit diff\n  umb-audit stats".to_string()
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Failure(String),
    Drift(String),
}

#[derive(Default)]
struct ListFilters {
    bucket: Option<String>,
    file: Option<String>,
    class: Option<String>,
}

impl ListFilters {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, CliError> {
        let mut args = args.peekable();
        let mut filters = Self::default();
        while let Some(arg) = args.next() {
            if arg == "-h" || arg == "--help" {
                println!("{}", usage());
                continue;
            }

            if let Some(value) = arg.strip_prefix("--bucket=") {
                validate_bucket(value)?;
                filters.bucket = Some(value.to_string());
                continue;
            }
            if arg == "--bucket" {
                let value = require_flag_value(&mut args, "--bucket")?;
                validate_bucket(&value)?;
                filters.bucket = Some(value);
                continue;
            }

            if let Some(value) = arg.strip_prefix("--file=") {
                filters.file = Some(value.to_string());
                continue;
            }
            if arg == "--file" {
                filters.file = Some(require_flag_value(&mut args, "--file")?);
                continue;
            }

            if let Some(value) = arg.strip_prefix("--class=") {
                validate_class(value)?;
                filters.class = Some(value.to_string());
                continue;
            }
            if arg == "--class" {
                let value = require_flag_value(&mut args, "--class")?;
                validate_class(&value)?;
                filters.class = Some(value);
                continue;
            }

            return Err(CliError::Usage(format!(
                "unknown list option `{arg}`\n\n{}",
                usage()
            )));
        }

        Ok(filters)
    }

    fn matches(&self, entry: &CsvEntry) -> bool {
        self.bucket
            .as_deref()
            .is_none_or(|bucket| entry.bucket == bucket)
            && self.file.as_deref().is_none_or(|file| entry.file == file)
            && self
                .class
                .as_deref()
                .is_none_or(|class| entry.expected_class == class)
    }
}

fn require_flag_value(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, CliError> {
    args.next()
        .filter(|value| !value.starts_with('-'))
        .ok_or_else(|| CliError::Usage(format!("missing value for {flag}")))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CsvEntry {
    id: String,
    file: String,
    line: String,
    kind: String,
    route: String,
    surface: String,
    bucket: String,
    expected_class: String,
    spec_anchor: String,
    upstream_gate: String,
    existing_fixture: String,
    notes: String,
}

impl CsvEntry {
    fn from_record(line_number: usize, record: Vec<String>) -> Result<Self, CliError> {
        let expected_len = csv_header_fields().len();
        if record.len() != expected_len {
            return Err(CliError::Failure(format!(
                "CSV record at line {line_number} has {} fields; expected {expected_len}",
                record.len()
            )));
        }

        Ok(Self {
            id: record[0].clone(),
            file: record[1].clone(),
            line: record[2].clone(),
            kind: record[3].clone(),
            route: record[4].clone(),
            surface: record[5].clone(),
            bucket: record[6].clone(),
            expected_class: record[7].clone(),
            spec_anchor: record[8].clone(),
            upstream_gate: record[9].clone(),
            existing_fixture: record[10].clone(),
            notes: record[11].clone(),
        })
    }

    fn field(&self, field: &str) -> &str {
        match field {
            "id" => &self.id,
            "file" => &self.file,
            "line" => &self.line,
            "kind" => &self.kind,
            "route" => &self.route,
            "surface" => &self.surface,
            "bucket" => &self.bucket,
            "expected_class" => &self.expected_class,
            "spec_anchor" => &self.spec_anchor,
            "upstream_gate" => &self.upstream_gate,
            "existing_fixture" => &self.existing_fixture,
            "notes" => &self.notes,
            _ => unreachable!("unknown inventory field {field}"),
        }
    }
}
