//! Manifest parsing tests covering DESIGN section 1.2's acceptance and
//! rejection matrix.

use crate::model::{ConeKind, DependencyLocator};
use crate::parse_manifest;
use scoop_identity::ConeCoordinate;

const MINIMAL: &str = r#"
schema = 1

[cone]
group = "dev.example"
name = "app"
version = "0.1.0"
kind = "library"
"#;

#[test]
fn minimal_library_manifest() {
    let manifest = parse_manifest(MINIMAL).expect("parses");
    assert_eq!(
        *manifest.coordinate(),
        ConeCoordinate::new("dev.example", "app", "0.1.0").unwrap()
    );
    assert_eq!(manifest.kind(), ConeKind::Library);
    assert!(manifest.dependencies().is_empty());
}

#[test]
fn executable_kind() {
    let text = MINIMAL.replace("\"library\"", "\"executable\"");
    let manifest = parse_manifest(&text).expect("parses");
    assert_eq!(manifest.kind(), ConeKind::Executable);
}

#[test]
fn all_three_locator_forms() {
    let text = r#"
schema = 1

[cone]
group = "dev.example"
name = "app"
version = "0.1.0"
kind = "executable"

[dependencies]
"org.foo:bar" = { version = "1.2.3", path = "../bar" }
"org.acme:util" = { version = "2.0.0", artifact = "../artifacts/util.slib" }
"org.other:log" = "3.1.0"
"#;
    let manifest = parse_manifest(text).expect("parses");
    let dependencies = manifest.dependencies();
    assert_eq!(dependencies.len(), 3);
    assert_eq!(
        dependencies[0].coordinate(),
        &ConeCoordinate::new("org.foo", "bar", "1.2.3").unwrap()
    );
    assert_eq!(
        dependencies[0].locator(),
        &DependencyLocator::SourcePath {
            path: "../bar".to_owned()
        }
    );
    assert_eq!(
        dependencies[1].locator(),
        &DependencyLocator::ArtifactPath {
            path: "../artifacts/util.slib".to_owned()
        }
    );
    assert_eq!(dependencies[2].locator(), &DependencyLocator::Search);
    // The semantic projection hides locators.
    let semantic: Vec<_> = manifest.semantic_dependency_coordinates().collect();
    assert_eq!(semantic.len(), 3);
}

#[test]
fn missing_required_fields() {
    let error = parse_manifest("").unwrap_err();
    assert!(error.iter().any(|d| d.message.contains("schema")));
    let no_cone = "schema = 1\n";
    let error = parse_manifest(no_cone).unwrap_err();
    assert!(error.iter().any(|d| d.message.contains("[cone]")));

    let missing_field = r#"
schema = 1
[cone]
group = "a"
name = "b"
version = "0.1.0"
"#;
    let error = parse_manifest(missing_field).unwrap_err();
    assert!(error.iter().any(|d| d.message.contains("`kind`")));
}

#[test]
fn unknown_fields_are_rejected_not_ignored() {
    let typo = MINIMAL.replace("kind", "knd");
    let error = parse_manifest(&typo).unwrap_err();
    assert!(error.iter().any(|d| d.message.contains("unknown field")));
    assert!(error.iter().any(|d| d.message.contains("`kind`")));

    let extra = format!("{MINIMAL}\n[cone.extra]\nx = 1\n");
    let error = parse_manifest(&extra).unwrap_err();
    assert!(error.iter().any(|d| d.message.contains("unknown field")));

    let typo_dep = format!(
        "{MINIMAL}\n[dependencies]\n\"org.foo:bar\" = {{ version = \"1.0.0\", paths = \"../x\" }}\n"
    );
    let error = parse_manifest(&typo_dep).unwrap_err();
    assert!(
        error
            .iter()
            .any(|d| d.message.contains("unknown field") || d.message.contains("allowed fields"))
    );
}

#[test]
fn schema_version_must_be_one() {
    for bad in ["schema = 2\n[cone]\n", "schema = \"1\"\n[cone]\n"] {
        let error = parse_manifest(bad).unwrap_err();
        assert!(
            error.iter().any(|d| d.message.contains("schema")),
            "{bad:?}: {error:?}"
        );
    }
}

#[test]
fn malformed_toml_reports_error() {
    let error = parse_manifest("schema = ").unwrap_err();
    assert!(error.iter().any(|d| d.message.contains("malformed TOML")));
}

#[test]
fn non_canonical_coordinates_are_rejected() {
    let bad_version = MINIMAL.replace("0.1.0", "v0.1.0");
    let error = parse_manifest(&bad_version).unwrap_err();
    assert!(error.iter().any(|d| d.message.contains("coordinate")));

    let bad_group = MINIMAL.replace("dev.example", "Dev.Example");
    assert!(parse_manifest(&bad_group).is_err());
}

#[test]
fn unknown_kind_is_rejected() {
    let bad = MINIMAL.replace("\"library\"", "\"lib\"");
    let error = parse_manifest(&bad).unwrap_err();
    assert!(error.iter().any(|d| d.message.contains("cone kind")));
}

#[test]
fn reserved_core_cannot_be_declared() {
    let text = format!("{MINIMAL}\n[dependencies]\n\"scoop:scoop.core\" = \"0.1.0\"\n");
    let error = parse_manifest(&text).unwrap_err();
    assert!(error.iter().any(|d| d.message.contains("reserved core")));

    // A triple-part key is malformed independent of the core name check.
    let malformed = format!("{MINIMAL}\n[dependencies]\n\"scoop:scoop.core:0.1.0\" = \"0.1.0\"\n");
    let error = parse_manifest(&malformed).unwrap_err();
    assert!(
        error
            .iter()
            .any(|d| d.message.contains("exactly one ':' separator")),
        "{error:?}"
    );
}

#[test]
fn dependency_key_and_version_validation() {
    let bad_key = format!("{MINIMAL}\n[dependencies]\n\"org.foo\" = \"1.0.0\"\n");
    let error = parse_manifest(&bad_key).unwrap_err();
    assert!(error.iter().any(|d| d.message.contains("group:name")));

    let bad_version = format!("{MINIMAL}\n[dependencies]\n\"org.foo:bar\" = \"^1.0.0\"\n");
    let error = parse_manifest(&bad_version).unwrap_err();
    assert!(
        error
            .iter()
            .any(|d| d.message.contains("dependency coordinate"))
    );
}

#[test]
fn both_locators_is_an_error() {
    let text = format!(
        "{MINIMAL}\n[dependencies]\n\"org.foo:bar\" = {{ version = \"1.0.0\", path = \"../b\", \
         artifact = \"b.slib\" }}\n"
    );
    let error = parse_manifest(&text).unwrap_err();
    assert!(
        error
            .iter()
            .any(|d| d.message.contains("at most one locator"))
    );
}

#[test]
fn empty_locator_strings_are_errors() {
    let text = format!(
        "{MINIMAL}\n[dependencies]\n\"org.foo:bar\" = {{ version = \"1.0.0\", path = \"\" }}\n"
    );
    let error = parse_manifest(&text).unwrap_err();
    assert!(
        error
            .iter()
            .any(|d| d.message.contains("must not be empty"))
    );
}

#[test]
fn diagnostics_carry_spans_when_available() {
    let error = parse_manifest(MINIMAL.replace("version", "versiom").as_str()).unwrap_err();
    assert!(
        error
            .iter()
            .any(|d| d.span.is_some() || d.message.contains("missing required field")),
        "{error:?}"
    );
}

#[test]
fn core_coordinate_is_valid_for_the_sysroot_manifest() {
    // The trusted sysroot Cone itself uses the reserved coordinate; only
    // *depending on* it from a user manifest is rejected.
    let text = r#"
schema = 1

[cone]
group = "scoop"
name = "scoop.core"
version = "0.1.0"
kind = "library"
"#;
    let manifest = parse_manifest(text).expect("sysroot core manifest parses");
    assert!(manifest.coordinate().is_reserved_core());
}
