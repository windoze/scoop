use super::*;
use crate::ReplayedLayoutLinkObjectContentsV1;

/// Object replay and physical imports from one owned, dependency-first read.
pub struct LinkObjectsReplayedCrossConeLayoutClosure {
    physical: PhysicalImportsReplayedCrossConeLayoutClosure,
    objects: Vec<ReplayedLayoutLinkObjectContentsV1>,
}

impl<'input> LirDependencyGraphReplayedCrossConeLayoutClosure<'input> {
    pub fn replay_link_object_contents(
        self,
        profile: &lir::CBridgeToolchainProfileV1,
    ) -> Result<LinkObjectsReplayedCrossConeLayoutClosure, CrossConeLayoutLirPhysicalError> {
        let (physical, objects) = self.replay_physical(|artifact, _, _, _| {
            artifact
                .prepared
                .replay_link_object_contents(&artifact.strong, profile)
        })?;
        Ok(LinkObjectsReplayedCrossConeLayoutClosure { physical, objects })
    }
}

impl LinkObjectsReplayedCrossConeLayoutClosure {
    pub const fn physical_imports(&self) -> &PhysicalImportsReplayedCrossConeLayoutClosure {
        &self.physical
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &PhysicalImportsReplayedCrossConeLayoutSections,
            &ReplayedLayoutLinkObjectContentsV1,
        ),
    > {
        self.physical.dependency_first().zip(&self.objects)
    }

    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<(
        &PhysicalImportsReplayedCrossConeLayoutSections,
        &ReplayedLayoutLinkObjectContentsV1,
    )> {
        self.physical
            .positions
            .get(&provider)
            .map(|&index| (&self.physical.artifacts[index], &self.objects[index]))
    }
}
