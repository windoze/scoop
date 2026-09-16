use std::path::Path;

use scoop_hir::CanonicalHirFoundation;
use scoop_identity::{ArtifactCapabilityProfileId, ConeCoordinate};
use scoop_lir::{CanonicalLirFoundation, ValidatedLirTargetSelection};
use scoop_mir::CanonicalMirFoundation;
use scoop_slib::{
    ConeKind, ConeRecord, ConeSourceForm, IdentityFoundationArtifact,
    IdentityFoundationArtifactInput, ProducerRecord,
};
use scoop_wire::{DecodeLimits, sha256};

use super::*;
use crate::{CacheReceiptBodyV1, PairedCompilerFingerprintV1, SnapshotFileError};

struct Fixture {
    artifact: ImmutableInputSnapshot,
    receipt: CacheReceiptV1,
}

fn key(seed: &[u8]) -> ConeCompileCacheKeyV1 {
    ConeCompileCacheKeyV1::from_digest(sha256(seed))
}

fn fixture(root: &Path, key: ConeCompileCacheKeyV1, producer: &str) -> Fixture {
    let hir = CanonicalHirFoundation::empty();
    let mir = CanonicalMirFoundation::empty();
    let lir = CanonicalLirFoundation::empty();
    let cone = ConeRecord::new(
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let foundation = IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new(producer).unwrap(),
        cone.clone(),
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        &hir,
        &mir,
        &lir,
    ))
    .unwrap();
    let path = root.join(format!("{producer}.slib"));
    std::fs::write(&path, foundation.as_bytes()).unwrap();
    let artifact = ImmutableInputSnapshot::capture(&path, u64::MAX).unwrap();
    let body = CacheReceiptBodyV1::new(
        key,
        foundation.artifact_fingerprint(),
        cone,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        Vec::new(),
        PairedCompilerFingerprintV1::from_parts(
            sha256(b"paired-scoopc"),
            sha256(b"distribution"),
            sha256(b"build"),
        ),
        ArtifactCapabilityProfileId::single_cone_strong(),
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

    assert_eq!(
        store.lookup(&lock, DecodeLimits::M23_DEFAULT).unwrap(),
        CompileCacheLookupV1::Miss
    );
    let published = store
        .publish(
            &lock,
            &fixture.artifact,
            &fixture.receipt,
            DecodeLimits::M23_DEFAULT,
        )
        .unwrap();
    assert!(matches!(published, CompileCachePublishV1::Published(_)));

    let path = store.entry_path(key);
    assert_eq!(path.file_name().unwrap().to_string_lossy().len(), 64);
    assert!(path.join(ARTIFACT_FILE_NAME).is_file());
    assert!(path.join(RECEIPT_FILE_NAME).is_file());
    let CompileCacheLookupV1::Hit(hit) = store.lookup(&lock, DecodeLimits::M23_DEFAULT).unwrap()
    else {
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
        .publish(
            &lock,
            &first.artifact,
            &first.receipt,
            DecodeLimits::M23_DEFAULT,
        )
        .unwrap();

    assert!(matches!(
        store
            .publish(
                &lock,
                &first.artifact,
                &first.receipt,
                DecodeLimits::M23_DEFAULT,
            )
            .unwrap(),
        CompileCachePublishV1::ExistingEquivalent(_)
    ));
    assert!(matches!(
        store.publish(
            &lock,
            &second.artifact,
            &second.receipt,
            DecodeLimits::M23_DEFAULT,
        ),
        Err(CompileCacheStoreError::NondeterministicProduction(_))
    ));
    assert_eq!(
        std::fs::read_dir(store.namespace_path().join(STAGING_DIRECTORY))
            .unwrap()
            .count(),
        0
    );
    let CompileCacheLookupV1::Hit(winner) = store.lookup(&lock, DecodeLimits::M23_DEFAULT).unwrap()
    else {
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
        .publish(
            &lock,
            &fixture.artifact,
            &fixture.receipt,
            DecodeLimits::M23_DEFAULT,
        )
        .unwrap();
    let limits = DecodeLimits {
        owned_bytes: 1,
        ..DecodeLimits::M23_DEFAULT
    };

    assert!(matches!(
        store.lookup(&lock, limits),
        Err(CompileCacheStoreError::Snapshot {
            role: CachePathRole::ReceiptFile,
            source: SnapshotFileError::TooLarge { .. },
        })
    ));
}

#[test]
fn exact_key_corruption_is_not_a_miss() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let key = key(b"corrupt");
    let fixture = fixture(temp.path(), key, "cache-corrupt");
    let lock = store.acquire_exclusive(key).unwrap();
    store
        .publish(
            &lock,
            &fixture.artifact,
            &fixture.receipt,
            DecodeLimits::M23_DEFAULT,
        )
        .unwrap();
    std::fs::write(store.entry_path(key).join("unexpected"), b"evidence").unwrap();

    assert!(matches!(
        store.lookup(&lock, DecodeLimits::M23_DEFAULT),
        Err(CompileCacheStoreError::UnexpectedEntryContents { .. })
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
        .publish(
            &lock,
            &fixture.artifact,
            &fixture.receipt,
            DecodeLimits::M23_DEFAULT,
        )
        .unwrap();
    let artifact_path = store.entry_path(key).join(ARTIFACT_FILE_NAME);
    std::fs::remove_file(&artifact_path).unwrap();
    symlink(fixture.artifact.source_locator(), &artifact_path).unwrap();
    assert!(matches!(
        store.lookup(&lock, DecodeLimits::M23_DEFAULT),
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
        .publish(
            &first_lock,
            &fixture.artifact,
            &fixture.receipt,
            DecodeLimits::M23_DEFAULT,
        )
        .unwrap();
    drop(first_lock);
    std::fs::rename(store.entry_path(first_key), store.entry_path(second_key)).unwrap();
    let second_lock = store.acquire_shared(second_key).unwrap();

    assert!(matches!(
        store.lookup(&second_lock, DecodeLimits::M23_DEFAULT),
        Err(CompileCacheStoreError::ReceiptKeyMismatch {
            expected,
            actual,
        }) if expected == second_key && actual == first_key
    ));
}
