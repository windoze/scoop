//! Owned member references into one validated artifact closure.

use std::rc::Rc;

use super::*;

/// Keeps the selected artifact's Compile, Link, and publication views in
/// their original closure without cloning or revalidating any view.
pub struct SharedCrossConeArtifact<'input> {
    closure: Rc<ValidatedCrossConeArtifactClosure<'input>>,
    position: usize,
}

impl<'input> ValidatedCrossConeArtifactClosure<'input> {
    pub fn share_artifact(
        self: &Rc<Self>,
        identity: ConeIdentity,
    ) -> Option<SharedCrossConeArtifact<'input>> {
        self.positions
            .get(&identity)
            .map(|position| SharedCrossConeArtifact {
                closure: Rc::clone(self),
                position: *position,
            })
    }
}

impl<'input> SharedCrossConeArtifact<'input> {
    pub fn compile(
        &self,
    ) -> &ValidatedCompileArtifact<'input, crate::CrossConeSemanticsStrongProfile> {
        self.closure.semantic.artifact_at(self.position)
    }

    pub fn link(&self) -> &ValidatedCrossConeStrongLinkArtifact<'input> {
        &self.closure.links[self.position]
    }

    pub fn publication(&self) -> &PublishableCrossConeArtifact {
        &self.closure.publications[self.position]
    }
}
