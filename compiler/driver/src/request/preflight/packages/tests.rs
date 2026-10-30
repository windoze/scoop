use super::*;
use crate::request::preflight::{parse_manifest_current, parse_single_file_current};
use scoop_manifest::{ManifestRootLocator, SingleFileLocator};
use scoop_protocol::CompilationPurpose;

fn manifest(directory: &Path, group: &str, name: &str) -> LoadedConeManifest {
    let path = directory.join("Cone.toml");
    std::fs::write(&path, format!(
        "schema = 1\n[cone]\ngroup = {group:?}\nname = {name:?}\nversion = \"0.1.0\"\nkind = \"library\"\n"
    )).unwrap();
    scoop_toolchain::load_cone_manifest(&ManifestRootLocator::exact_manifest_file(path)).unwrap()
}

use std::path::Path;

fn parsed(
    group: &str,
    name: &str,
    text: &str,
) -> Result<CurrentConeParsedSources, CurrentConeSourceStageError> {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("src")).unwrap();
    std::fs::write(directory.path().join("src/main.scoop"), text).unwrap();
    parse_manifest_current(
        &manifest(directory.path(), group, name),
        scoop_target_data::DARWIN_AARCH64_ID,
        CompilationPurpose::Production,
    )
}

#[test]
fn accepts_group_and_subpackages_as_actual_identifier_segments() {
    for (group, source) in [
        ("dev.example", "package dev.example\n"),
        ("dev.example", "package dev.example.http.client\n"),
        ("dev.my-team", "package dev.`my-team`.api\n"),
        ("dev.if", "package dev.`if`\n"),
    ] {
        parsed(group, "package-test", source).unwrap();
    }
}

#[test]
fn rejects_missing_or_unrelated_packages_without_string_prefix_matching() {
    for text in [
        "package dev.examples",
        "package dev",
        "package `dev.example`",
        "fun value() = 1",
        "",
    ] {
        let CurrentConeSourceStageError::Package(diagnostics) =
            parsed("dev.example", "package-test", text).unwrap_err()
        else {
            panic!("expected a package diagnostic for {text:?}")
        };
        assert_eq!(diagnostics.diagnostics().len(), 1);
        let diagnostic = &diagnostics.diagnostics()[0];
        assert_eq!(diagnostic.file, 0);
        assert!(diagnostic.message.contains("Cone group `dev.example`"));
        let span = diagnostic.span.unwrap();
        if text.starts_with("package") {
            assert_eq!(&text[span.start as usize..span.end as usize], text);
        } else {
            assert_eq!(span.start, span.end);
        }
    }
}

#[test]
fn only_core_and_std_are_exempt_from_manifest_package_requirements() {
    for name in ["scoop.core", "scoop.std"] {
        for source in ["fun value() = 1", "package unrelated.api\nfun value() = 1"] {
            parsed("scoop", name, source).unwrap();
            assert!(parsed("other", name, source).is_err());
        }
    }
    assert!(parsed("scoop", "scoop.cli", "fun value() = 1").is_err());
}

#[test]
fn selected_test_sources_follow_the_same_package_rule() {
    let directory = tempfile::tempdir().unwrap();
    for (path, text) in [
        ("src/main.scoop", "package dev.example"),
        ("tests/main.scoop", "package other"),
    ] {
        let path = directory.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let manifest = manifest(directory.path(), "dev.example", "package-test");
    parse_manifest_current(
        &manifest,
        scoop_target_data::DARWIN_AARCH64_ID,
        CompilationPurpose::Production,
    )
    .unwrap();
    let CurrentConeSourceStageError::Package(diagnostics) = parse_manifest_current(
        &manifest,
        scoop_target_data::DARWIN_AARCH64_ID,
        CompilationPurpose::Test,
    )
    .unwrap_err() else {
        panic!("test sources must be checked")
    };
    let test_path = Path::new("tests").join("main.scoop");
    assert!(
        diagnostics
            .render_human()
            .contains(&format!("{}:1:1", test_path.display()))
    );
}

#[test]
fn single_file_warning_preserves_original_text_and_syntax() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("script.scoop");
    let text = "#!scoop run\npackage other.api\nfun main() {}\n";
    std::fs::write(&path, text).unwrap();
    let locator = SingleFileLocator::from_path(path.clone()).unwrap();
    let sources = parse_single_file_current(&locator).unwrap();
    assert_eq!(sources.iter().next().unwrap().text().text(), text);
    assert!(matches!(
        sources.iter().next().unwrap().source().ast().package,
        PackageSyntax::QualifiedPackage { .. }
    ));
    let warnings = single_file_warnings(&sources);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].span, Some(Span::new(12, 29)));
    std::fs::write(&path, "package .bad\n").unwrap();
    assert!(matches!(
        parse_single_file_current(&locator),
        Err(CurrentConeSourceStageError::Parser(_))
    ));
}
