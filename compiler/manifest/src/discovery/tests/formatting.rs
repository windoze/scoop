use super::*;

#[test]
fn formatting_unions_all_conditions_and_roles_and_deduplicates_paths() {
    let cone = TempCone::new();
    cone.write("src/a.scoop", "fun a() {}");
    cone.write("linux/b.scoop", "fun b() {}");
    cone.write("tests/c.scoop", "fun c() {}");
    cone.select("[[sources]]\npath = 'src'\n[[sources]]\npath = 'src/a.scoop'\nwhen = {os = 'darwin'}\n[[sources]]\npath = 'linux'\nwhen = {os = 'linux'}");
    let manifest = cone.load();
    let paths = select_manifest_format_sources(&manifest).unwrap();
    assert_eq!(
        paths,
        ["linux/b.scoop", "src/a.scoop", "tests/c.scoop"]
            .map(|path| manifest.real_root().join(path))
    );
    assert!(select_manifest_sources(&manifest, scoop_target_data::DARWIN_AARCH64_ID).is_err());
}

#[test]
fn formatting_allows_empty_sets_but_checks_inactive_explicit_paths() {
    let cone = TempCone::new();
    cone.select("sources = []");
    assert!(
        select_manifest_format_sources(&cone.load())
            .unwrap()
            .is_empty()
    );
    let cone = TempCone::new();
    cone.select("[[sources]]\npath = 'missing'\nwhen = {os = []}");
    assert!(select_manifest_format_sources(&cone.load()).is_err());
}

#[test]
fn formatting_requires_default_production_and_explicit_test_roots() {
    let cone = TempCone::new();
    std::fs::remove_dir(cone.0.join("src")).unwrap();
    assert!(select_manifest_format_sources(&cone.load()).is_err());
    let cone = TempCone::new();
    cone.write("src/api.scoop", "fun api() {}");
    let manifest = cone.0.join("Cone.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        manifest,
        format!("{text}\n[test]\nsources = [{{path = 'missing'}}]\n"),
    )
    .unwrap();
    assert!(select_manifest_format_sources(&cone.load()).is_err());
}

#[cfg(unix)]
#[test]
fn formatting_resolves_aliases_to_one_physical_file() {
    let cone = TempCone::new();
    cone.write("src/a.scoop", "fun a() {}");
    std::os::unix::fs::symlink("a.scoop", cone.0.join("src/b.scoop")).unwrap();
    assert_eq!(
        select_manifest_format_sources(&cone.load()).unwrap().len(),
        1
    );
}
