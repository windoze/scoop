use std::rc::Rc;
use std::sync::Arc;

use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::{ArtifactManifestSummaryError, ArtifactManifestSummaryV1, ArtifactSnapshot};

/// Immutable bytes and manifest data retained by the build scheduler.
#[derive(Debug)]
pub struct BuildArtifact {
    pub(super) snapshot: Arc<ArtifactSnapshot>,
    pub(super) summary: ArtifactManifestSummaryV1,
}

impl BuildArtifact {
    pub(crate) fn read(
        snapshot: Arc<ArtifactSnapshot>,
        target: ValidatedLirTargetSelection,
    ) -> Result<Rc<Self>, ArtifactManifestSummaryError> {
        let summary = snapshot.manifest_summary(target)?;
        Ok(Rc::new(Self { snapshot, summary }))
    }

    pub fn snapshot(&self) -> &Arc<ArtifactSnapshot> {
        &self.snapshot
    }

    pub const fn summary(&self) -> &ArtifactManifestSummaryV1 {
        &self.summary
    }
}
