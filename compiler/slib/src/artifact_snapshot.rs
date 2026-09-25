//! Immutable artifact bytes shared by cache, reader, and child transport.

use std::io::{self, Write};
use std::sync::Arc;

use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::{Digest256, sha256};

use crate::{PrebuiltManifestSummaryError, probe_prebuilt_manifest_summary};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSnapshot {
    bytes: Arc<[u8]>,
    digest: Digest256,
}

impl ArtifactSnapshot {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self::from_shared(bytes.into())
    }

    pub fn from_shared(bytes: Arc<[u8]>) -> Self {
        let digest = sha256(&bytes);
        Self { bytes, digest }
    }

    pub const fn digest(&self) -> Digest256 {
        self.digest
    }

    pub fn byte_length(&self) -> usize {
        self.bytes.len()
    }

    /// Returns the immutable bytes used by artifact readers and child builds.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn probe_prebuilt_summary(
        &self,

        target: ValidatedLirTargetSelection,
    ) -> Result<crate::PrebuiltManifestSummaryV1, PrebuiltManifestSummaryError> {
        probe_prebuilt_manifest_summary(self.as_bytes(), target)
    }

    pub fn write_to(&self, writer: &mut impl Write) -> io::Result<()> {
        writer.write_all(self.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immutable_snapshot_preserves_artifact_bytes() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(false);
        let snapshot = ArtifactSnapshot::from_bytes(bytes.clone());
        let summary = snapshot
            .probe_prebuilt_summary(ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1)
            .unwrap();
        let mut materialized = Vec::new();
        snapshot.write_to(&mut materialized).unwrap();
        assert_eq!(materialized, bytes);
        assert_eq!(summary.artifact_fingerprint().as_array().len(), 32);
    }
}
