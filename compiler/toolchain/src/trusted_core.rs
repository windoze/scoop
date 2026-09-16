use std::path::{Path, PathBuf};

use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::{Encoder, WireEncode};

const CORE_SOURCE_RELATIVE_PATH: &str = "lib/scoop.core";
const CORE_ARTIFACTS_RELATIVE_PATH: &str = "artifacts";
const CORE_ARTIFACT_FILE_NAME: &str = "scoop.core.slib";
const CORE_RECEIPT_FILE_NAME: &str = "scoop.core.receipt.cbor";
const CORE_LOCK_FILE_NAME: &str = "scoop.core.lock";
const CORE_BOOTSTRAP_PROFILE_MAGIC: &str = "scoop-trusted-core-bootstrap";

/// Closed identity of the trusted core bootstrap semantics consumed by the
/// paired compiler. It is a cache-key input, not intrinsic authority by
/// itself.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TrustedCoreBootstrapProfileIdV1;

impl TrustedCoreBootstrapProfileIdV1 {
    pub const CURRENT: Self = Self;
}

impl WireEncode for TrustedCoreBootstrapProfileIdV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.text(CORE_BOOTSTRAP_PROFILE_MAGIC)?;
        encoder.field(2)?;
        encoder.unsigned(1)
    }
}

/// Pure target-qualified sysroot layout shared by the orchestration parent and
/// the paired single-Cone compiler. This type grants no filesystem authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedCoreSlotLayoutV1 {
    source_root: PathBuf,
    artifact_root: PathBuf,
    artifact: PathBuf,
    receipt: PathBuf,
    lock: PathBuf,
}

impl TrustedCoreSlotLayoutV1 {
    pub fn new(sysroot: &Path, target: ValidatedLirTargetSelection) -> Self {
        let artifact_root = sysroot
            .join(CORE_ARTIFACTS_RELATIVE_PATH)
            .join(target.target().id().canonical_name());
        Self {
            source_root: sysroot.join(CORE_SOURCE_RELATIVE_PATH),
            artifact: artifact_root.join(CORE_ARTIFACT_FILE_NAME),
            receipt: artifact_root.join(CORE_RECEIPT_FILE_NAME),
            lock: artifact_root.join(CORE_LOCK_FILE_NAME),
            artifact_root,
        }
    }

    pub fn source_root(&self) -> &Path {
        &self.source_root
    }

    pub fn artifact_root(&self) -> &Path {
        &self.artifact_root
    }

    pub fn artifact(&self) -> &Path {
        &self.artifact
    }

    pub fn receipt(&self) -> &Path {
        &self.receipt
    }

    pub fn lock(&self) -> &Path {
        &self.lock
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_qualified_layout_has_one_shared_spelling() {
        let layout = TrustedCoreSlotLayoutV1::new(
            Path::new("/opt/scoop"),
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        );
        assert_eq!(layout.source_root(), Path::new("/opt/scoop/lib/scoop.core"));
        assert_eq!(
            layout.artifact_root(),
            Path::new("/opt/scoop/artifacts/darwin-aarch64")
        );
        assert_eq!(
            layout.artifact(),
            Path::new("/opt/scoop/artifacts/darwin-aarch64/scoop.core.slib")
        );
        assert_eq!(
            layout.receipt(),
            Path::new("/opt/scoop/artifacts/darwin-aarch64/scoop.core.receipt.cbor")
        );
        assert_eq!(
            layout.lock(),
            Path::new("/opt/scoop/artifacts/darwin-aarch64/scoop.core.lock")
        );
    }

    #[test]
    fn bootstrap_profile_has_one_frozen_wire_identity() {
        assert_eq!(
            scoop_wire::encode(&TrustedCoreBootstrapProfileIdV1::CURRENT).unwrap(),
            [
                0xa2, 0x01, 0x78, 0x1c, b's', b'c', b'o', b'o', b'p', b'-', b't', b'r', b'u', b's',
                b't', b'e', b'd', b'-', b'c', b'o', b'r', b'e', b'-', b'b', b'o', b'o', b't', b's',
                b't', b'r', b'a', b'p', 0x02, 0x01,
            ]
        );
    }
}
