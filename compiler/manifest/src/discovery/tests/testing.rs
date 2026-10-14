use super::*;

fn discover(cone: &TempCone) -> Result<DiscoveredManifestSources, SourceDiscoveryError> {
    discover_manifest_sources_for_tests(&cone.load(), scoop_target_data::DARWIN_AARCH64_ID)
}

#[test]
fn combined_sources_keep_roles_identities_and_global_path_order() {
    let cone = TempCone::new();
    cone.write("src/api.scoop", "fun api() {}\n");
    cone.write("cases/test.scoop", "fun test() {}\n");
    cone.write("tests/unused.scoop", "not selected");
    cone.select("[[test.sources]]\npath = 'cases'");
    let production =
        discover_manifest_sources(&cone.load(), scoop_target_data::DARWIN_AARCH64_ID).unwrap();
    let combined = discover(&cone).unwrap();
    let files = combined.iter().collect::<Vec<_>>();
    assert_eq!(files.len(), 2);
    assert_eq!(
        files[0].identity().logical_path().as_str(),
        "cases/test.scoop"
    );
    assert_eq!(files[0].role(), SourceRole::Test);
    assert_eq!(files[1], production.first());
    assert_eq!(files[1].role(), SourceRole::Production);
    assert!(
        files
            .iter()
            .all(|source| source.identity().cone() == cone.load().identity())
    );
}

#[test]
fn default_test_directory_is_optional_and_empty_tests_are_valid() {
    let cone = TempCone::new();
    cone.write("src/api.scoop", "fun api() {}\n");
    assert_eq!(discover(&cone).unwrap().iter().count(), 1);
    std::fs::create_dir(cone.0.join("tests")).unwrap();
    let empty = discover(&cone).unwrap();
    assert_eq!(empty.iter().count(), 1);
    assert!(
        empty
            .selected_directories()
            .contains(&ConeRelativePath::new("tests").unwrap())
    );
    cone.write("tests/test.scoop", "fun test() {}\n");
    assert_eq!(discover(&cone).unwrap().iter().count(), 2);
    cone.select("[test]\nsources = []");
    assert_eq!(discover(&cone).unwrap().iter().count(), 1);
}

#[test]
fn tests_do_not_complete_an_empty_production_source_set() {
    let cone = TempCone::new();
    cone.write("tests/test.scoop", "fun test() {}\n");
    assert!(matches!(
        discover(&cone).unwrap_err().kind(),
        SourceDiscoveryErrorKind::EmptySourceSet
    ));
    cone.write("src/unused.scoop", "fun api() {}\n");
    cone.select("sources = []");
    assert!(matches!(
        discover(&cone).unwrap_err().kind(),
        SourceDiscoveryErrorKind::EmptySourceSet
    ));
}

#[test]
fn test_platform_selection_does_not_read_unselected_files() {
    let cone = TempCone::new();
    cone.write("src/api.scoop", "fun api() {}\n");
    cone.write("cases/common.scoop", "fun common() {}\n");
    for target in crate::test_catalog::SOURCE_TARGETS {
        cone.write(&format!("{}/test.scoop", target.env), "fun test() {}\n");
    }
    cone.select(
        "[[test.sources]]\npath = 'cases'\n\
        [[test.sources]]\npath = 'none'\nwhen = {os = 'darwin'}\n\
        [[test.sources]]\npath = 'gnu'\nwhen = {env = 'gnu'}\n\
        [[test.sources]]\npath = 'musl'\nwhen = {env = 'musl'}\n\
        [[test.sources]]\npath = 'missing'\nwhen = {os = []}",
    );
    for target in crate::test_catalog::SOURCE_TARGETS {
        let sources = discover_manifest_sources_for_tests(&cone.load(), target.id).unwrap();
        let tests = sources
            .iter()
            .filter(|s| s.role() == SourceRole::Test)
            .map(|s| s.identity().logical_path().as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            tests,
            ["cases/common.scoop", &format!("{}/test.scoop", target.env)]
        );
    }
}

#[test]
fn selected_test_paths_and_contents_are_checked_only_for_test_compilation() {
    for (selection, path, bytes) in [
        (
            "[[test.sources]]\npath = 'missing'",
            "unused",
            b"".as_slice(),
        ),
        ("", "tests", b"not a directory".as_slice()),
        ("", "tests/bad.scoop", &[0xff]),
    ] {
        let cone = TempCone::new();
        cone.write("src/api.scoop", "fun api() {}\n");
        cone.write(path, "");
        std::fs::write(cone.0.join(path), bytes).unwrap();
        cone.select(selection);
        discover_manifest_sources(&cone.load(), scoop_target_data::DARWIN_AARCH64_ID).unwrap();
        let error = discover(&cone).unwrap_err();
        assert!(matches!(
            error.kind(),
            SourceDiscoveryErrorKind::Io { .. }
                | SourceDiscoveryErrorKind::TestRootNotDirectory
                | SourceDiscoveryErrorKind::InvalidUtf8Source
        ));
    }
}

#[test]
fn rejects_overlap_between_production_and_test_selections() {
    for path in ["src", "src/nested", "src/nested/api.scoop"] {
        let cone = TempCone::new();
        cone.write("src/nested/api.scoop", "fun api() {}\n");
        cone.select(&format!("[[test.sources]]\npath = '{path}'"));
        let error = discover(&cone).unwrap_err();
        let SourceDiscoveryErrorKind::ConflictingSelections { first, second } = error.kind() else {
            panic!("expected conflicting selections, got {error}");
        };
        assert_eq!(first, "default src/");
        assert!(second.starts_with("test.sources[1]"));
    }
}

#[cfg(unix)]
#[test]
fn rejects_test_symlink_aliases_broken_links_and_escape() {
    let cone = TempCone::new();
    cone.write("src/api.scoop", "fun api() {}\n");
    std::os::unix::fs::symlink("src", cone.0.join("tests")).unwrap();
    assert!(matches!(
        discover(&cone).unwrap_err().kind(),
        SourceDiscoveryErrorKind::ConflictingSelections { .. }
    ));
    std::fs::remove_file(cone.0.join("tests")).unwrap();
    cone.write("tests/local.scoop", "fun local() {}\n");
    let alias = cone.0.join("tests/alias.scoop");
    std::os::unix::fs::symlink("../src/api.scoop", &alias).unwrap();
    assert!(matches!(
        discover(&cone).unwrap_err().kind(),
        SourceDiscoveryErrorKind::ConflictingSelections { .. }
    ));
    std::fs::remove_file(alias).unwrap();
    std::fs::remove_dir_all(cone.0.join("tests")).unwrap();
    std::os::unix::fs::symlink("missing", cone.0.join("tests")).unwrap();
    assert!(matches!(
        discover(&cone).unwrap_err().kind(),
        SourceDiscoveryErrorKind::Io { .. }
    ));
    std::fs::remove_file(cone.0.join("tests")).unwrap();
    let outside = TempCone::new();
    std::os::unix::fs::symlink(&outside.0, cone.0.join("tests")).unwrap();
    assert!(matches!(
        discover(&cone).unwrap_err().kind(),
        SourceDiscoveryErrorKind::EscapesConeRoot
    ));
}
