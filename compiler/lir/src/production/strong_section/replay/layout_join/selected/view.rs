use crate::{
    CanonicalExternalShapeLinkImportsV1, LayoutAbiDependencyV1, LayoutAbiSemanticTargetV1,
    PhysicalImportsReplayedLayoutAbiSectionV1, SelectedDependencyLayoutAbiSetV1,
};
use scoop_identity::ConeIdentity;

#[derive(Clone, Copy)]
pub(in crate::production::strong_section::replay::layout_join) enum Selection<'s, 'p> {
    Complete(&'s SelectedDependencyLayoutAbiSetV1<'p>),
    Replayed(&'s PhysicalImportsReplayedLayoutAbiSectionV1),
}

impl<'s, 'p> Selection<'s, 'p> {
    pub(super) fn consumer(self) -> ConeIdentity {
        match self {
            Self::Complete(value) => value.consumer(),
            Self::Replayed(value) => value.exports().provider(),
        }
    }

    pub(super) fn contains(
        self,
        provider: ConeIdentity,
        target: LayoutAbiSemanticTargetV1,
    ) -> bool {
        match self {
            Self::Complete(value) => value.reference(provider, target).is_some(),
            Self::Replayed(value) => value
                .selected_relations()
                .binary_search(&LayoutAbiDependencyV1::new(provider, target))
                .is_ok(),
        }
    }

    pub(super) fn physical(self) -> &'s CanonicalExternalShapeLinkImportsV1 {
        match self {
            Self::Complete(value) => value.physical_imports(),
            Self::Replayed(value) => value.physical_imports(),
        }
    }
}
