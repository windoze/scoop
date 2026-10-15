use crate::{
    CanonicalExternalShapeLinkImportsV1, LayoutAbiSemanticTargetV1,
    SelectedDependencyLayoutAbiSetV1,
};
use scoop_identity::ConeIdentity;

#[derive(Clone, Copy)]
pub(in crate::production::cone_section::layout_join) struct Selection<'s, 'p>(
    pub &'s SelectedDependencyLayoutAbiSetV1<'p>,
);
impl<'s, 'p> Selection<'s, 'p> {
    pub(super) fn consumer(self) -> ConeIdentity {
        self.0.consumer()
    }
    pub(super) fn contains(
        self,
        provider: ConeIdentity,
        target: LayoutAbiSemanticTargetV1,
    ) -> bool {
        self.0.record(provider, target).is_some()
    }
    pub(super) fn physical(self) -> &'s CanonicalExternalShapeLinkImportsV1 {
        self.0.physical_imports()
    }
}
