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
    let slot = resolve_trusted_core_slot_at(&sysroot.0, target()).unwrap();
    assert_eq!(slot.artifact(), sysroot.artifact_path());
    assert_eq!(slot.source_root(), sysroot.0.join("lib/scoop.core"));
    assert!(!slot.source_root().exists());
}
