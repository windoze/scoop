use std::path::Path;

use scoop_identity::{ArtifactCapabilityProfileId, ConeCoordinate};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::{ConeKind, ConeRecord, ConeSourceForm};
use scoop_wire::sha256;

use super::*;
use crate::{CacheReceiptBodyV1, PairedCompilerFingerprintV1};

struct Fixture {
    artifact: ImmutableInputSnapshot,
    receipt: CacheReceiptV1,
}

fn key(seed: &[u8]) -> ConeCompileCacheKeyV1 {
    ConeCompileCacheKeyV1::from_digest(sha256(seed))
}

fn fixture(root: &Path, key: ConeCompileCacheKeyV1, producer: &str) -> Fixture {
    let cone = ConeRecord::new(
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let archive = crate::test_artifacts::manifest_archive(
        scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        cone.clone(),
        producer,
        Vec::new(),
    );
    let summary = scoop_slib::read_artifact_manifest_summary(
        archive.as_bytes(),
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    let path = root.join(format!("{producer}.slib"));
    std::fs::write(&path, archive.as_bytes()).unwrap();
    let artifact = ImmutableInputSnapshot::capture(&path).unwrap();
    let body = CacheReceiptBodyV1::new(
        key,
        summary.artifact_fingerprint(),
        cone,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        Vec::new(),
        PairedCompilerFingerprintV1::from_parts(
            sha256(b"paired-scoopc"),
            sha256(b"distribution"),
            sha256(b"build"),
        ),
        ArtifactCapabilityProfileId::cross_cone_generic(),
        Vec::new(),
    )
    .unwrap();
    Fixture {
        artifact,
        receipt: CacheReceiptV1::new(body).unwrap(),
    }
}

fn store(root: &Path) -> CompileCacheStoreV1 {
    CompileCacheStoreV1::new(&ArtifactCacheRoot::new(root.join("cache")).unwrap())
}

#[test]
fn cache_publishes_and_reads_one_exact_atomic_entry() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let key = key(b"round-trip");
    let fixture = fixture(temp.path(), key, "cache-round-trip");
    let lock = store.acquire_exclusive(key).unwrap();

    assert_eq!(store.lookup(&lock).unwrap(), CompileCacheLookupV1::Miss);
    let published = store
        .publish(&lock, &fixture.artifact, &fixture.receipt)
        .unwrap();
    assert!(matches!(published, CompileCachePublishV1::Published(_)));

    let path = store.entry_path(key);
    assert_eq!(path.file_name().unwrap().to_string_lossy().len(), 64);
    assert!(path.join(ARTIFACT_FILE_NAME).is_file());
    assert!(path.join(RECEIPT_FILE_NAME).is_file());
    let CompileCacheLookupV1::Hit(hit) = store.lookup(&lock).unwrap() else {
        panic!("published entry must be a hit");
    };
    assert_eq!(hit.key(), key);
    assert_eq!(hit.artifact().as_bytes(), fixture.artifact.as_bytes());
    assert_eq!(hit.receipt(), &fixture.receipt);
}

#[test]
fn cache_never_overwrites_an_existing_winner() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let key = key(b"winner");
    let first = fixture(temp.path(), key, "cache-first");
    let second = fixture(temp.path(), key, "cache-second");
    let lock = store.acquire_exclusive(key).unwrap();
    store
        .publish(&lock, &first.artifact, &first.receipt)
        .unwrap();

    let equivalent = store
        .publish(&lock, &first.artifact, &first.receipt)
        .unwrap();
    assert!(matches!(
        equivalent,
        CompileCachePublishV1::ExistingEquivalent(_)
    ));

    assert!(matches!(
        store.publish(&lock, &second.artifact, &second.receipt,),
        Err(CompileCacheStoreError::NondeterministicProduction(_))
    ));
    assert_eq!(
        std::fs::read_dir(store.namespace_path().join(STAGING_DIRECTORY))
            .unwrap()
            .count(),
        0
    );
    let CompileCacheLookupV1::Hit(winner) = store.lookup(&lock).unwrap() else {
        panic!("winner must remain visible");
    };
    assert_eq!(winner.artifact().as_bytes(), first.artifact.as_bytes());
}

#[test]
fn cache_payload_reads_are_bounded_and_never_degrade_to_miss() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let key = key(b"bounded");
    let fixture = fixture(temp.path(), key, "cache-bounded");
    let lock = store.acquire_exclusive(key).unwrap();
    store
        .publish(&lock, &fixture.artifact, &fixture.receipt)
        .unwrap();
}

#[test]
fn exact_key_corruption_is_not_a_miss() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let key = key(b"corrupt");
    let fixture = fixture(temp.path(), key, "cache-corrupt");
    let lock = store.acquire_exclusive(key).unwrap();
    store
        .publish(&lock, &fixture.artifact, &fixture.receipt)
        .unwrap();
    std::fs::write(store.entry_path(key).join("unexpected"), b"evidence").unwrap();

    assert!(matches!(
        store.lookup(&lock),
        Err(CompileCacheStoreError::UnexpectedEntryContents { .. })
    ));
}

#[test]
fn exact_key_missing_or_truncated_files_are_not_misses() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());

    for (seed, producer, missing_file) in [
        (
            b"missing-receipt".as_slice(),
            "cache-missing-receipt",
            RECEIPT_FILE_NAME,
        ),
        (
            b"missing-artifact".as_slice(),
            "cache-missing-artifact",
            ARTIFACT_FILE_NAME,
        ),
    ] {
        let key = key(seed);
        let fixture = fixture(temp.path(), key, producer);
        let lock = store.acquire_exclusive(key).unwrap();
        store
            .publish(&lock, &fixture.artifact, &fixture.receipt)
            .unwrap();
        std::fs::remove_file(store.entry_path(key).join(missing_file)).unwrap();

        assert!(matches!(
            store.lookup(&lock),
            Err(CompileCacheStoreError::UnexpectedEntryContents { .. })
        ));
    }

    let key = key(b"truncated-receipt");
    let fixture = fixture(temp.path(), key, "cache-truncated-receipt");
    let lock = store.acquire_exclusive(key).unwrap();
    store
        .publish(&lock, &fixture.artifact, &fixture.receipt)
        .unwrap();
    let receipt_path = store.entry_path(key).join(RECEIPT_FILE_NAME);
    std::fs::remove_file(&receipt_path).unwrap();
    std::fs::write(receipt_path, [0xa1]).unwrap();

    assert!(matches!(
        store.lookup(&lock),
        Err(CompileCacheStoreError::ReceiptDecode(_))
    ));
}

#[cfg(unix)]
#[test]
fn cache_rejects_symlinked_payloads_and_lock_files() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let key = key(b"symlink");
    let fixture = fixture(temp.path(), key, "cache-symlink");
    let lock = store.acquire_exclusive(key).unwrap();
    let lock_path = lock.path().to_path_buf();
    store
        .publish(&lock, &fixture.artifact, &fixture.receipt)
        .unwrap();
    let artifact_path = store.entry_path(key).join(ARTIFACT_FILE_NAME);
    std::fs::remove_file(&artifact_path).unwrap();
    symlink(fixture.artifact.source_locator(), &artifact_path).unwrap();
    assert!(matches!(
        store.lookup(&lock),
        Err(CompileCacheStoreError::InvalidPathType {
            role: CachePathRole::ArtifactFile,
            ..
        })
    ));
    drop(lock);

    std::fs::remove_file(&lock_path).unwrap();
    symlink(fixture.artifact.source_locator(), &lock_path).unwrap();
    assert!(matches!(
        store.acquire_shared(key),
        Err(CompileCacheStoreError::InvalidPathType {
            role: CachePathRole::LockFile,
            ..
        })
    ));
}

#[test]
fn exclusive_lock_is_held_by_the_open_file_descriptor() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let key = key(b"lock");
    let lock = store.acquire_exclusive(key).unwrap();
    let other = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock.path())
        .unwrap();

    assert!(matches!(
        fs4::FileExt::try_lock(&other),
        Err(fs4::TryLockError::WouldBlock)
    ));
}

#[test]
fn receipt_must_name_the_exact_directory_key() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let first_key = key(b"first-key");
    let second_key = key(b"second-key");
    let fixture = fixture(temp.path(), first_key, "cache-key-mismatch");
    let first_lock = store.acquire_exclusive(first_key).unwrap();
    store
        .publish(&first_lock, &fixture.artifact, &fixture.receipt)
        .unwrap();
    drop(first_lock);
    std::fs::rename(store.entry_path(first_key), store.entry_path(second_key)).unwrap();
    let second_lock = store.acquire_shared(second_key).unwrap();

    assert!(matches!(
        store.lookup(&second_lock),
        Err(CompileCacheStoreError::ReceiptKeyMismatch {
            expected,
            actual,
        }) if expected == second_key && actual == first_key
    ));
}
