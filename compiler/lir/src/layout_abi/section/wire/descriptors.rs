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
    pub fn validate_descriptors(
        self,
        expected: &crate::CanonicalExactDescriptorExportsV1,
    ) -> Result<DescriptorsResolvedCrossConeLayoutAbiSectionV1, crate::ExactDescriptorTableError>
    {
        if self.layouts.provider() != expected.provider() {
            return Err(crate::ExactDescriptorTableError::LayoutProvider);
        }
        if self.layouts.target() != expected.target() {
            return Err(crate::ExactDescriptorTableError::LayoutTarget);
        }
        Ok(DescriptorsResolvedCrossConeLayoutAbiSectionV1 {
            layouts: self.layouts,
            descriptors: self.descriptors.validate_against(expected)?,
            dispatch: self.dispatch,
            callables: self.callables,
            shape_support: self.shape_support,
            selected: self.selected,
        })
    }
}
