use super::*;

impl<T>
    UnselectedCrossConeLayoutAbiSectionV1<
        crate::CanonicalExactLayoutExportsV1,
        crate::CanonicalExactCallableAbiExportsV1,
        crate::CanonicalExactDispatchExportsV1,
        T,
    >
{
    pub const fn dispatch(&self) -> &crate::CanonicalExactDispatchExportsV1 {
        &self.dispatch
    }
}

impl DispatchResolvedCrossConeLayoutAbiSectionV1 {
    pub fn read_descriptors(
        self,
        target: crate::LirTargetProfile,
        foundation: &crate::ConeLirFoundation,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<DescriptorsResolvedCrossConeLayoutAbiSectionV1, crate::LinkDataError> {
        let descriptors = self
            .descriptors
            .read(target, foundation, &self.layouts, identities)?;
        Ok(DescriptorsResolvedCrossConeLayoutAbiSectionV1 {
            layouts: self.layouts,
            descriptors,
            dispatch: self.dispatch,
            callables: self.callables,
            shape_support: self.shape_support,
            selected: self.selected,
        })
    }
}
