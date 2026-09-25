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

    pub fn validate<'a, E>(
        self,
        expected: &LayoutAbiExportConstituentsV1,
        dependencies: &[&'a CrossConeLayoutAbiSectionV1<'a>],
        physical_imports: Vec<crate::ExternalShapeLinkImportV1<'a>>,
        source: &impl LayoutAbiSectionSourceAuthorityV1<E>,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError<E>> {
        if self.layouts.provider() != expected.provider()
            || self.layouts.target() != expected.target_profile()
        {
            return Err(LayoutAbiSectionError::LayoutReplayChanged);
        }
        self.validate_descriptors(expected.descriptors())?.validate(
            expected,
            dependencies,
            physical_imports,
            source,
            identities,
        )
    }
}
