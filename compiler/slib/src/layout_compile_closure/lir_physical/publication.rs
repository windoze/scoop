//! Publication borrows metadata from the common reader result.

use super::*;

pub(crate) struct LayoutPublicationParts<'a> {
    pub manifest: &'a crate::BootstrapManifest,
    pub meter: &'a mut BudgetMeter,
}

impl PhysicalImportsReplayedCrossConeLayoutSections<'_, '_> {
    pub(crate) fn publication_parts(&mut self) -> LayoutPublicationParts<'_> {
        let parts = self.prepared.semantic_parts();
        LayoutPublicationParts {
            manifest: parts.manifest,
            meter: parts.meter,
        }
    }
}
