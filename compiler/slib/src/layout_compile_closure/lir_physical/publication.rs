//! Publication borrows metadata from the common reader result.

use super::*;

pub(crate) struct LayoutPublicationParts<'a> {
    pub manifest: &'a crate::BootstrapManifest,
}

impl PhysicalImportsReplayedCrossConeLayoutSections {
    pub(crate) fn publication_parts(&self) -> LayoutPublicationParts<'_> {
        LayoutPublicationParts {
            manifest: self.manifest(),
        }
    }
}
