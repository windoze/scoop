use super::*;

impl TempCone {
    fn select(&self, entries: &str) {
        let manifest = self.0.join("Cone.toml");
        let current = std::fs::read_to_string(&manifest).unwrap();
        std::fs::write(
            manifest,
            current.replacen("[cone]", &format!("{entries}\n[cone]"), 1),
        )
        .unwrap();
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

#[test]
fn each_target_selects_only_its_complete_source_set() {
    let cone = TempCone::new();
    cone.write("src/ignored.scoop", "invalid source outside explicit list");
    cone.write("common/api.scoop", "fun api() {}\n");
    for target in TargetProfileId::ALL {
        cone.write(&format!("{}/os.scoop", target.env()), "fun platform() {}\n");
    }
    cone.select(
        "[[sources]]\npath = 'common'\n\
                 [[sources]]\npath = 'none'\nwhen = {os = 'darwin'}\n\
                 [[sources]]\npath = 'gnu'\nwhen = {env = 'gnu'}\n\
                 [[sources]]\npath = 'musl'\nwhen = {env = 'musl'}\n\
                 [[sources]]\npath = 'does-not-exist'\nwhen = {os = []}\n",
    );
    for target in TargetProfileId::ALL {
        let sources = discover_manifest_sources(&cone.load(), target).unwrap();
        let paths: Vec<_> = sources
            .iter()
            .map(|s| s.identity().logical_path().as_str())
            .collect();
        assert_eq!(
            paths,
            ["common/api.scoop", &format!("{}/os.scoop", target.env())]
        );
    }
}

#[test]
fn all_empty_explicit_forms_do_not_fall_back_to_src() {
    for selection in [
        "sources = []",
        "[[sources]]\npath = 'missing'\nwhen = {os = []}",
        "[[sources]]\npath = 'empty'",
    ] {
        let cone = TempCone::new();
        cone.write("src/main.scoop", "fun main() {}\n");
        std::fs::create_dir(cone.0.join("empty")).unwrap();
        cone.select(selection);
        assert!(matches!(
            discover_manifest_sources(&cone.load(), TargetProfileId::DarwinAarch64)
                .unwrap_err()
                .kind(),
            SourceDiscoveryErrorKind::EmptySourceSet
        ));
    }
}

#[test]
fn selected_empty_directories_are_preserved_for_snapshot_materialization() {
    let cone = TempCone::new();
    cone.write("api.scoop", "fun api() {}\n");
    std::fs::create_dir(cone.0.join("empty")).unwrap();
    cone.select("[[sources]]\npath = 'api.scoop'\n[[sources]]\npath = 'empty'");
    let sources = discover_manifest_sources(&cone.load(), TargetProfileId::DarwinAarch64).unwrap();
    assert_eq!(
        sources.selected_directories(),
        &[ConeRelativePath::new("empty").unwrap()]
    );
    assert_eq!(
        sources.first().identity().logical_path().as_str(),
        "api.scoop"
    );
}

#[test]
fn rejects_normalized_duplicates_nested_roots_and_file_directory_overlap() {
    for paths in [
        ["src", "./src"],
        ["src", "src/sub"],
        ["src/sub/a.scoop", "src"],
    ] {
        let cone = TempCone::new();
        cone.write("src/sub/a.scoop", "fun a() {}\n");
        cone.select(&format!(
            "[[sources]]\npath = '{}'\n[[sources]]\npath = '{}'",
            paths[0], paths[1]
        ));
        let error =
            discover_manifest_sources(&cone.load(), TargetProfileId::DarwinAarch64).unwrap_err();
        assert!(matches!(
            error.kind(),
            SourceDiscoveryErrorKind::ConflictingSelections { .. }
        ));
        assert!(error.to_string().contains("sources[1]"));
        assert!(error.to_string().contains("sources[2]"));
    }
}

#[test]
fn mutually_exclusive_entries_can_select_the_same_path() {
    let cone = TempCone::new();
    cone.write("src/api.scoop", "fun api() {}\n");
    let baseline = discover_manifest_sources(&cone.load(), TargetProfileId::DarwinAarch64).unwrap();
    cone.select("[[sources]]\npath = './src'\nwhen = {os = 'darwin'}\n[[sources]]\npath = 'src'\nwhen = {os = 'linux'}");
    for target in TargetProfileId::ALL {
        let selected = discover_manifest_sources(&cone.load(), target).unwrap();
        assert_eq!(selected.first().identity(), baseline.first().identity());
    }
}

#[cfg(unix)]
#[test]
fn rejects_selected_symlinks_outside_the_cone_and_duplicate_aliases() {
    let cone = TempCone::new();
    let outside = TempCone::new();
    outside.write("api.scoop", "fun api() {}\n");
    std::os::unix::fs::symlink(outside.0.join("api.scoop"), cone.0.join("src/escape.scoop"))
        .unwrap();
    assert!(matches!(
        discover_manifest_sources(&cone.load(), TargetProfileId::DarwinAarch64)
            .unwrap_err()
            .kind(),
        SourceDiscoveryErrorKind::EscapesConeRoot
    ));
    std::fs::remove_file(cone.0.join("src/escape.scoop")).unwrap();
    cone.write("api.scoop", "fun api() {}\n");
    std::os::unix::fs::symlink("../api.scoop", cone.0.join("src/alias.scoop")).unwrap();
    cone.select("[[sources]]\npath = 'api.scoop'\n[[sources]]\npath = 'src'");
    assert!(matches!(
        discover_manifest_sources(&cone.load(), TargetProfileId::DarwinAarch64)
            .unwrap_err()
            .kind(),
        SourceDiscoveryErrorKind::ConflictingSelections { .. }
    ));
}
