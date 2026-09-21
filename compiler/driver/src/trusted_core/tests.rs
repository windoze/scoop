
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "scoop-trusted-core-slot-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(std::fs::canonicalize(path).unwrap())
    }

    fn write_core_manifest(&self, coordinate: (&str, &str, &str)) {
        let source = TrustedCoreSlotLayoutV1::new(&self.0, target())
            .source_root()
            .to_path_buf();
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(
                source.join("Cone.toml"),
                format!(
                    "schema = 1\n[cone]\ngroup = {:?}\nname = {:?}\nversion = {:?}\nkind = \"library\"\n",
                    coordinate.0, coordinate.1, coordinate.2
                ),
            )
            .unwrap();
    }

    fn artifact_path(&self) -> PathBuf {
        TrustedCoreSlotLayoutV1::new(&self.0, target())
            .artifact()
            .to_path_buf()
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn target() -> ValidatedLirTargetSelection {
    ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
}

#[test]
fn resolver_returns_default_source_and_artifact_locations() {
    let sysroot = TempDirectory::new();
    sysroot.write_core_manifest(("scoop", "scoop.core", "0.1.0"));
    let slot = resolve_trusted_core_slot_at(&sysroot.0, target()).unwrap();
    assert_eq!(slot.artifact().path(), sysroot.artifact_path());
    assert_eq!(
        slot.source().manifest().parsed().semantic().coordinate(),
        &ConeCoordinate::reserved_core()
    );
    assert_eq!(slot.artifact().target(), target());
}

#[test]
fn resolver_rejects_a_non_core_source_manifest() {
    let sysroot = TempDirectory::new();
    sysroot.write_core_manifest(("dev.example", "fake", "1.0.0"));

    assert!(matches!(
        resolve_trusted_core_slot_at(&sysroot.0, target())
            .unwrap_err()
            .kind(),
        TrustedCoreSlotErrorKind::SourceManifest(error)
            if matches!(
                error.kind(),
                scoop_manifest::ManifestRootErrorKind::Parse(parse)
                    if parse.kind()
                        == &scoop_manifest::ManifestParseErrorKind::TrustedCoreCoordinateMismatch
            )
    ));
}

#[test]
fn artifact_input_accepts_a_regular_file_outside_the_default_location() {
    let sysroot = TempDirectory::new();
    sysroot.write_core_manifest(("scoop", "scoop.core", "0.1.0"));
    let configured = sysroot.artifact_path();
    std::fs::create_dir_all(configured.parent().unwrap()).unwrap();
    std::fs::write(&configured, b"configured core").unwrap();
    let other = sysroot.0.join("other.slib");
    std::fs::write(&other, b"rebuilt core").unwrap();

    let slot = resolve_trusted_core_slot_at(&sysroot.0, target()).unwrap();
    let input = slot.existing_artifact_input().unwrap();
    assert_eq!(input.path(), configured);
    assert_eq!(
        input.expected_coordinate(),
        &ConeCoordinate::reserved_core()
    );
    assert_eq!(input.target(), target());
    let input = TrustedCoreArtifactInput::new(&other, target()).unwrap();
    assert_eq!(input.path(), other);
    assert_eq!(
        input.expected_coordinate(),
        &ConeCoordinate::reserved_core()
    );
}
