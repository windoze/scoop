use super::*;
use scoop_manifest::{ManifestRootErrorKind, SourceSelection};

const MANIFEST: &str = r#"schema = 1
sources = [{path = "src", when = {os = ["linux", "darwin"]}}]
[cone]
group = "test.cache"
name = "manifest"
version = "1.0.0"
kind = "library"
[dependencies]
"test:source" = {version = "1.0.0", path = "../source"}
"test:artifact" = {version = "1.0.0", artifact = "../artifact.slib"}
"test:search" = "1.0.0"
[test]
sources = [{path = "checks"}]
[test-dependencies]
"test:assert" = {version = "1.0.0", path = "../assert"}
[native]
include = ["native/include"]
c_flags = ["-D", "VALUE=4", "-iquote", "native/quoted"]
cxx_flags = ["-std=c++17"]
cxx = true
sources = [{path = "native/input.c", when = {env = ["gnu", "musl"]}}]
libraries = [{name = "m", kind = "dynamic", when = {os = "linux"}}]
"#;

#[test]
fn parsed_record_preserves_configuration_and_diagnostic_spans() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Cone.toml");
    std::fs::write(&path, MANIFEST).unwrap();
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(
            std::fs::FileTimes::new()
                .set_modified(std::time::UNIX_EPOCH - std::time::Duration::from_secs(1)),
        )
        .unwrap();
    let cache = ArtifactCacheRoot::new(temp.path().join("cache")).unwrap();
    let locator = ManifestRootLocator::cone_directory(temp.path());
    let cold = cache.load_manifest(&locator).unwrap();
    let warm = cache.load_manifest(&locator).unwrap();
    assert_eq!(cold.parsed(), warm.parsed());
    assert_eq!(cold.source_text(), warm.source_text());
    assert_eq!(cold.identity(), warm.identity());
    assert_eq!(cold.real_root(), warm.real_root());
    let semantic = warm.parsed().semantic();
    let SourceSelection::Explicit(paths) = semantic.sources() else {
        panic!("explicit sources")
    };
    assert!(
        paths[0]
            .predicate()
            .matches(scoop_target_data::DARWIN_AARCH64_ID)
    );
    assert_eq!(
        semantic
            .native()
            .library_requirements(scoop_target_data::LINUX_X86_64_GNU_ID)
            .unwrap()
            .len(),
        1
    );
    let (_, dependency) = warm.parsed().tests().dependency_iter().next().unwrap();
    let span = dependency.spans().path().unwrap().range();
    assert_eq!(&warm.source_text()[span], "\"../assert\"");
}

#[test]
fn changed_manifest_reparses_without_invalidating_unchanged_production_fields() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Cone.toml");
    std::fs::write(&path, MANIFEST).unwrap();
    let cache = ArtifactCacheRoot::new(temp.path().join("cache")).unwrap();
    let locator = ManifestRootLocator::exact_manifest_file(&path);
    let old = cache.load_manifest(&locator).unwrap();
    std::fs::write(&path, MANIFEST.replace("checks", "new-checks")).unwrap();
    let changed = cache.load_manifest(&locator).unwrap();
    assert_eq!(old.parsed().semantic(), changed.parsed().semantic());
    assert_ne!(old.parsed().tests(), changed.parsed().tests());

    std::fs::write(&path, MANIFEST.replace("library", "executable")).unwrap();
    let changed = cache.load_manifest(&locator).unwrap();
    assert_eq!(
        changed.parsed().semantic().requested_kind(),
        scoop_manifest::RequestedConeKind::Executable
    );
}

#[test]
fn invalid_or_missing_manifest_is_an_input_error_and_does_not_publish_a_record() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Cone.toml");
    std::fs::write(&path, "schema = 1\n[cone]\n").unwrap();
    let cache = ArtifactCacheRoot::new(temp.path().join("cache")).unwrap();
    let locator = ManifestRootLocator::cone_directory(temp.path());
    assert!(matches!(
        cache.load_manifest(&locator).unwrap_err().kind(),
        ManifestRootErrorKind::Parse(_)
    ));
    assert!(
        std::fs::read_dir(cache.as_path().join("manifests-v2"))
            .unwrap()
            .all(|entry| entry.unwrap().path().extension().unwrap() == "lock")
    );
    std::fs::write(&path, MANIFEST).unwrap();
    cache.load_manifest(&locator).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(
        matches!(cache.load_manifest(&locator).unwrap_err().kind(), ManifestRootErrorKind::Io { source, .. } if source.kind() == ErrorKind::NotFound)
    );
}
