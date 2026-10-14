use super::*;
use scoop_identity::ConeCoordinate;
use scoop_slib::{ConeKind, ConeRecord, ConeSourceForm};
use scoop_wire::sha256;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn key(seed: &[u8]) -> ConeCompileCacheKeyV1 {
    ConeCompileCacheKeyV1::from_digest(sha256(seed))
}

fn fixture() -> (Vec<u8>, CompileBuildRecordV1) {
    let archive = crate::test_artifacts::manifest_archive(
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        "build-record",
        Vec::new(),
    );
    let summary = scoop_slib::read_artifact_manifest_summary(
        archive.as_bytes(),
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    (
        archive.as_bytes().to_vec(),
        CompileBuildRecordV1 {
            summary,
            warnings: Vec::new(),
        },
    )
}

fn store(root: &Path) -> CompileCacheStoreV1 {
    CompileCacheStoreV1::new(&ArtifactCacheRoot::new(root.join("cache")).unwrap())
}

#[test]
fn published_result_is_reused_and_missing_output_requires_update() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let lock = store.acquire_exclusive(key(b"reuse")).unwrap();
    let (bytes, record) = fixture();
    let output = temp.path().join("output.slib");
    assert!(matches!(
        store.lookup(&lock).unwrap(),
        CompileCacheLookupV1::Miss
    ));
    std::fs::write(&output, &bytes).unwrap();
    let result = store.publish(&lock, &output, record.clone()).unwrap();
    let CompileCacheLookupV1::Hit(reused) = store.lookup(&lock).unwrap() else {
        panic!("published result missing")
    };
    assert_eq!(reused.artifact, result.artifact);
    assert_eq!(reused.record, record);
    std::fs::remove_file(&result.artifact).unwrap();
    assert!(matches!(
        store.lookup(&lock).unwrap(),
        CompileCacheLookupV1::Miss
    ));
    std::fs::write(&output, &bytes).unwrap();
    let rebuilt = store.publish(&lock, &output, record).unwrap();
    assert_eq!(std::fs::read(rebuilt.artifact).unwrap(), bytes);
}

#[test]
fn failed_output_publication_does_not_publish_a_build_record() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let lock = store.acquire_exclusive(key(b"failed-output")).unwrap();
    let (_, record) = fixture();
    assert!(
        store
            .publish(&lock, &temp.path().join("absent.slib"), record)
            .is_err()
    );
    assert!(matches!(
        store.lookup(&lock).unwrap(),
        CompileCacheLookupV1::Miss
    ));
}

#[test]
fn concurrent_builds_produce_one_result_under_the_key_lock() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let (bytes, record) = fixture();
    let produced = Arc::new(AtomicUsize::new(0));
    std::thread::scope(|scope| {
        let handles = (0..4)
            .map(|index| {
                let store = &store;
                let bytes = &bytes;
                let record = record.clone();
                let produced = Arc::clone(&produced);
                let output = temp.path().join(format!("candidate-{index}.slib"));
                scope.spawn(move || {
                    let lock = store.acquire_exclusive(key(b"concurrent")).unwrap();
                    if let CompileCacheLookupV1::Hit(entry) = store.lookup(&lock).unwrap() {
                        return entry.artifact;
                    }
                    produced.fetch_add(1, Ordering::SeqCst);
                    std::fs::write(&output, bytes).unwrap();
                    store.publish(&lock, &output, record).unwrap().artifact
                })
            })
            .collect::<Vec<_>>();
        let paths = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert!(paths.windows(2).all(|pair| pair[0] == pair[1]));
    });
    assert_eq!(produced.load(Ordering::SeqCst), 1);
}
