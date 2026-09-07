//! Strict `Cone.toml` parsing with spanned diagnostics.

use std::fmt;
use std::ops::Range;

use scoop_identity::ConeCoordinate;
use toml_edit::{DocumentMut, Item, Table, TomlError, Value};

use crate::MANIFEST_SCHEMA_VERSION;
use crate::model::{ConeKind, ConeManifest, DeclaredDependency, DependencyLocator};

/// One manifest error: a message plus, when known, the byte range in the
/// manifest text that produced it. Manifest errors are never attributed
/// to source files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestDiagnostic {
    pub message: String,
    pub span: Option<Range<usize>>,
}

impl ManifestDiagnostic {
    fn at(message: impl Into<String>, span: Option<Range<usize>>) -> Self {
        ManifestDiagnostic {
            message: message.into(),
            span,
        }
    }
}

impl fmt::Display for ManifestDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ManifestDiagnostic {}

/// Parses one `Cone.toml`. Structural errors (malformed TOML, missing
/// `[cone]`) fail fast; field-level problems within an otherwise readable
/// document are collected together.
pub fn parse_manifest(text: &str) -> Result<ConeManifest, Vec<ManifestDiagnostic>> {
    let document: DocumentMut = text.parse().map_err(|error: TomlError| {
        vec![ManifestDiagnostic::at(
            format!("malformed TOML: {error}"),
            error.span(),
        )]
    })?;
    parse_document(&document)
}

fn parse_document(document: &DocumentMut) -> Result<ConeManifest, Vec<ManifestDiagnostic>> {
    let mut errors = Vec::new();

    for (key, item) in document.iter() {
        if !matches!(key, "schema" | "cone" | "dependencies") {
            errors.push(ManifestDiagnostic::at(
                format!("unknown manifest field or table {key:?}"),
                item.span(),
            ));
        }
    }

    let schema = document.get("schema");
    match schema {
        None => errors.push(ManifestDiagnostic::at(
            "missing required field `schema`",
            None,
        )),
        Some(item) => {
            let span = item.span();
            match item.as_value().and_then(Value::as_integer) {
                Some(value) if value == MANIFEST_SCHEMA_VERSION => {}
                Some(value) => errors.push(ManifestDiagnostic::at(
                    format!(
                        "unsupported manifest schema {value}; this toolchain accepts \
                         schema {MANIFEST_SCHEMA_VERSION}"
                    ),
                    span,
                )),
                None => errors.push(ManifestDiagnostic::at(
                    "field `schema` must be an integer",
                    span,
                )),
            }
        }
    }

    let cone = match document.get("cone").and_then(Item::as_table) {
        Some(table) => table,
        None => {
            errors.push(ManifestDiagnostic::at(
                "missing required table `[cone]`",
                None,
            ));
            return Err(errors);
        }
    };

    let coordinate = parse_coordinate(cone, &mut errors);
    let kind = parse_kind(cone, &mut errors);

    let dependencies = match document.get("dependencies") {
        None => Vec::new(),
        Some(item) => match item.as_table() {
            Some(table) => parse_dependencies(table, &mut errors),
            None => {
                errors.push(ManifestDiagnostic::at(
                    "`[dependencies]` must be a table",
                    item.span(),
                ));
                Vec::new()
            }
        },
    };

    match (coordinate, kind) {
        (Some(coordinate), Some(kind)) if errors.is_empty() => {
            Ok(ConeManifest::new(coordinate, kind, dependencies))
        }
        _ => Err(errors),
    }
}

fn parse_coordinate(cone: &Table, errors: &mut Vec<ManifestDiagnostic>) -> Option<ConeCoordinate> {
    reject_unknown_fields(cone, &["group", "name", "version", "kind"], errors);
    let group = required_string(cone, "group", errors);
    let name = required_string(cone, "name", errors);
    let version = required_string(cone, "version", errors);
    let (Some(group), Some(name), Some(version)) = (group, name, version) else {
        return None;
    };
    match ConeCoordinate::new(&group, &name, &version) {
        Ok(coordinate) => Some(coordinate),
        Err(error) => {
            errors.push(ManifestDiagnostic::at(
                format!("invalid cone coordinate: {error}"),
                cone.get("version").and_then(Item::span).or(cone.span()),
            ));
            None
        }
    }
}

fn parse_kind(cone: &Table, errors: &mut Vec<ManifestDiagnostic>) -> Option<ConeKind> {
    let kind = required_string(cone, "kind", errors)?;
    match kind.as_str() {
        "library" => Some(ConeKind::Library),
        "executable" => Some(ConeKind::Executable),
        other => {
            errors.push(ManifestDiagnostic::at(
                format!("unknown cone kind {other:?}; expected \"library\" or \"executable\""),
                cone.get("kind").and_then(Item::span),
            ));
            None
        }
    }
}

fn parse_dependencies(
    table: &Table,
    errors: &mut Vec<ManifestDiagnostic>,
) -> Vec<DeclaredDependency> {
    let mut dependencies = Vec::new();
    for (key, item) in table.iter() {
        let span = item.span();
        let (group, name) = match split_dependency_key(key) {
            Ok(parts) => parts,
            Err(message) => {
                errors.push(ManifestDiagnostic::at(message, span));
                continue;
            }
        };
        if group == "scoop" && name == "scoop.core" {
            errors.push(ManifestDiagnostic::at(
                "cannot declare a dependency on the reserved core coordinate \
                 `scoop:scoop.core`; it is injected implicitly from the trusted sysroot",
                span,
            ));
            continue;
        }
        match item {
            Item::Value(Value::String(version)) => {
                match ConeCoordinate::new(group, name, version.value()) {
                    Ok(coordinate) => dependencies.push(DeclaredDependency::new(
                        coordinate,
                        DependencyLocator::Search,
                    )),
                    Err(error) => errors.push(ManifestDiagnostic::at(
                        format!("invalid dependency coordinate: {error}"),
                        span,
                    )),
                }
            }
            Item::Table(_) | Item::Value(Value::InlineTable(_)) => {
                match parse_dependency_entry(group, name, item, errors) {
                    Some(dependency) => dependencies.push(dependency),
                    None => continue,
                }
            }
            _ => errors.push(ManifestDiagnostic::at(
                "dependency value must be a version string or a table",
                span,
            )),
        }
    }
    dependencies
}

/// A dependency entry is either an inline table or a sub-table; both
/// forms carry the same fields.
fn entry_field<'a>(item: &'a Item, field: &str) -> Option<&'a Value> {
    match item {
        Item::Table(table) => table.get(field)?.as_value(),
        Item::Value(Value::InlineTable(table)) => table.get(field),
        _ => None,
    }
}

fn entry_field_names(item: &Item) -> Vec<String> {
    match item {
        Item::Table(table) => table.iter().map(|(key, _)| key.to_owned()).collect(),
        Item::Value(Value::InlineTable(table)) => {
            table.iter().map(|(key, _)| key.to_owned()).collect()
        }
        _ => Vec::new(),
    }
}

fn parse_dependency_entry(
    group: &str,
    name: &str,
    entry: &Item,
    errors: &mut Vec<ManifestDiagnostic>,
) -> Option<DeclaredDependency> {
    for field in entry_field_names(entry) {
        if !matches!(field.as_str(), "version" | "path" | "artifact") {
            errors.push(ManifestDiagnostic::at(
                format!(
                    "unknown dependency field {field:?}; allowed fields are `version`, \
                     `path`, `artifact`"
                ),
                entry_field(entry, &field)
                    .and_then(Value::span)
                    .or(entry.span()),
            ));
        }
    }
    let version = entry_string(entry, "version", true, errors)?;
    let path = entry_string(entry, "path", false, errors);
    let artifact = entry_string(entry, "artifact", false, errors);
    let coordinate = ConeCoordinate::new(group, name, &version).ok()?;
    let locator = match (path, artifact) {
        (None, None) => DependencyLocator::Search,
        (Some(path), None) => DependencyLocator::SourcePath { path },
        (None, Some(artifact)) => DependencyLocator::ArtifactPath { path: artifact },
        (Some(_), Some(_)) => {
            errors.push(ManifestDiagnostic::at(
                "dependency declares both `path` and `artifact`; at most one locator \
                 is allowed",
                entry.span(),
            ));
            return None;
        }
    };
    Some(DeclaredDependency::new(coordinate, locator))
}

/// Reads one string field of a dependency entry. `required` selects
/// between the two missing-field diagnostics; empty strings are always
/// rejected.
#[allow(clippy::type_complexity)]
fn entry_string(
    entry: &Item,
    field: &str,
    required: bool,
    errors: &mut Vec<ManifestDiagnostic>,
) -> Option<String> {
    let item = entry_field(entry, field);
    match item {
        None => {
            if required {
                errors.push(ManifestDiagnostic::at(
                    format!("missing required dependency field `{field}`"),
                    entry.span(),
                ));
            }
            None
        }
        Some(value) => {
            let span = value.span();
            match value.as_str() {
                Some(value) if !value.is_empty() => Some(value.to_owned()),
                Some(_) => {
                    errors.push(ManifestDiagnostic::at(
                        format!("dependency field `{field}` must not be empty"),
                        span,
                    ));
                    None
                }
                None => {
                    errors.push(ManifestDiagnostic::at(
                        format!("dependency field `{field}` must be a string"),
                        span,
                    ));
                    None
                }
            }
        }
    }
}

fn reject_unknown_fields(table: &Table, allowed: &[&str], errors: &mut Vec<ManifestDiagnostic>) {
    for (key, item) in table.iter() {
        if !allowed.contains(&key) {
            errors.push(ManifestDiagnostic::at(
                format!(
                    "unknown field {key:?}; allowed fields are {}",
                    allowed
                        .iter()
                        .map(|name| format!("`{name}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                item.span(),
            ));
        }
    }
}

fn required_string(
    table: &Table,
    field: &str,
    errors: &mut Vec<ManifestDiagnostic>,
) -> Option<String> {
    let item = table.get(field);
    let span = item.and_then(Item::span).or(table.span());
    match item {
        None => {
            errors.push(ManifestDiagnostic::at(
                format!("missing required field `{field}`"),
                table.span(),
            ));
            None
        }
        Some(item) => match item.as_value().and_then(Value::as_str) {
            Some(value) if !value.is_empty() => Some(value.to_owned()),
            Some(_) => {
                errors.push(ManifestDiagnostic::at(
                    format!("field `{field}` must not be empty"),
                    span,
                ));
                None
            }
            None => {
                errors.push(ManifestDiagnostic::at(
                    format!("field `{field}` must be a string"),
                    span,
                ));
                None
            }
        },
    }
}

fn split_dependency_key(key: &str) -> Result<(&str, &str), String> {
    let (group, name) = key
        .split_once(':')
        .ok_or_else(|| format!("dependency key {key:?} must be `group:name`"))?;
    if group.is_empty() || name.is_empty() {
        return Err(format!(
            "dependency key {key:?} must have non-empty group and name"
        ));
    }
    if name.contains(':') {
        return Err(format!(
            "dependency key {key:?} must contain exactly one ':' separator"
        ));
    }
    Ok((group, name))
}
