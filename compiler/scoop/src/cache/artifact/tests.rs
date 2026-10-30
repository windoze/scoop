use super::*;
use scoop_identity::ConeCoordinate;
use scoop_slib::{ArtifactCapabilityProfile, ConeKind, ConeRecord, ConeSourceForm};
use std::time::{Duration, UNIX_EPOCH};

const TARGET: ValidatedLirTargetSelection = scoop_target_data::DARWIN_AARCH64_LLVM_22_1;

fn write_artifact(path: &Path, producer: &str, modified: std::time::SystemTime) {
    let cone = ConeRecord::new(
        ConeCoordinate::new("test", "summary", "1.0.0").unwrap(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let archive = crate::test_artifacts::manifest_archive(
        TARGET,
        ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        cone,
        producer,
        Vec::new(),
    );
    std::fs::write(path, archive.as_bytes()).unwrap();
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
}

fn records(cache: &ArtifactCacheRoot) -> Vec<(std::path::PathBuf, std::time::SystemTime)> {
    let mut records = std::fs::read_dir(cache.as_path().join("artifact-summaries-v3"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "cbor")
        })
        .map(|path| {
            let modified = path.metadata().unwrap().modified().unwrap();
            (path, modified)
        })
        .collect::<Vec<_>>();
    records.sort();
    records
}

#[test]
fn summaries_reuse_records_and_follow_actual_artifact_updates() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("library.slib");
    let cache = ArtifactCacheRoot::new(temp.path().join("cache")).unwrap();
    write_artifact(&path, "first", UNIX_EPOCH - Duration::from_secs(1));
    let first = cache.load_artifact(&path, TARGET).unwrap();
    let before = records(&cache);
    assert_eq!(before.len(), 1);
    assert_eq!(cache.load_artifact(&path, TARGET).unwrap(), first);
    assert_eq!(records(&cache), before);
    write_artifact(&path, "second", UNIX_EPOCH + Duration::from_secs(1));
    let second = cache.load_artifact(&path, TARGET).unwrap();
    assert_eq!(first.summary().cone(), second.summary().cone());
    assert_ne!(
        first.summary().artifact_fingerprint(),
        second.summary().artifact_fingerprint()
    );
    assert_eq!(records(&cache).len(), 2);
}

#[test]
fn incompatible_target_and_missing_input_do_not_publish_summary_records() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("library.slib");
    let cache = ArtifactCacheRoot::new(temp.path().join("cache")).unwrap();
    write_artifact(&path, "first", UNIX_EPOCH);
    cache.load_artifact(&path, TARGET).unwrap();
    let before = records(&cache);
    assert!(matches!(
        cache.load_artifact(&path, scoop_target_data::LINUX_X86_64_GNU_LLVM_22_1),
        Err(DependencyLocatorError::Summary { .. })
    ));
    assert_eq!(records(&cache), before);
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(cache.load_artifact(&path, TARGET),
        Err(DependencyLocatorError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound));
    assert_eq!(records(&cache), before);
}
