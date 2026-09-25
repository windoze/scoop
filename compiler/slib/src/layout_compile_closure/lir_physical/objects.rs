use super::*;
use crate::ReplayedLayoutLinkObjectContentsV1;

/// Object replay and physical imports from one owned, dependency-first read.
/// This state cannot publish code or bypass the remaining source/use joins.
pub struct LinkObjectsReplayedCrossConeLayoutClosure<'checked, 'input> {
    physical: PhysicalImportsReplayedCrossConeLayoutClosure<'checked, 'input>,
    objects: Vec<ReplayedLayoutLinkObjectContentsV1<'input>>,
}

impl<'input> LirDependencyGraphReplayedCrossConeLayoutClosure<'input> {
    pub fn with_replayed_link_object_contents<R>(
        self,
        profile: &lir::CBridgeToolchainProfileV1,
        use_checked: impl for<'checked> FnOnce(
            LinkObjectsReplayedCrossConeLayoutClosure<'checked, 'input>,
        ) -> R,
    ) -> Result<R, CrossConeLayoutLirPhysicalError> {
        self.with_replayed_physical(
            |artifact, _, _, _| {
                artifact
                    .prepared
                    .replay_link_object_contents(&artifact.strong, profile)
            },
            |physical, objects| {
                use_checked(LinkObjectsReplayedCrossConeLayoutClosure { physical, objects })
            },
        )
    }
}

impl<'checked, 'input> LinkObjectsReplayedCrossConeLayoutClosure<'checked, 'input> {
    pub const fn physical_imports(
        &self,
    ) -> &PhysicalImportsReplayedCrossConeLayoutClosure<'checked, 'input> {
        &self.physical
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &'checked PhysicalImportsReplayedCrossConeLayoutSections<'input, 'checked>,
            &ReplayedLayoutLinkObjectContentsV1<'input>,
        ),
    > {
        self.physical.dependency_first().zip(&self.objects)
    }

    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<(
        &'checked PhysicalImportsReplayedCrossConeLayoutSections<'input, 'checked>,
        &ReplayedLayoutLinkObjectContentsV1<'input>,
    )> {
        self.physical
            .positions
            .get(&provider)
            .map(|&index| (self.physical.artifacts[index], &self.objects[index]))
    }
}
