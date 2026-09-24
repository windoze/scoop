use super::*;

impl DecodedCrossConeLayoutAbiSectionV1 {
    pub fn validate_layouts(
        self,
        expected: &crate::CanonicalExactLayoutExportsV1,
        meter: &mut BudgetMeter,
    ) -> Result<LayoutsResolvedCrossConeLayoutAbiSectionV1, crate::ExactLayoutTableError> {
        Ok(LayoutsResolvedCrossConeLayoutAbiSectionV1 {
            layouts: self.layouts.validate_against(expected, meter)?,
            descriptors: self.descriptors,
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
        meter: &mut BudgetMeter,
    ) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError<E>> {
        self.validate_layouts(expected.layouts(), meter)?.validate(
            expected,
            dependencies,
            physical_imports,
            source,
            identities,
            meter,
        )
    }
}

impl<C, D> UnselectedCrossConeLayoutAbiSectionV1<crate::CanonicalExactLayoutExportsV1, C, D> {
    pub const fn layouts(&self) -> &crate::CanonicalExactLayoutExportsV1 {
        &self.layouts
    }
}

impl LayoutsResolvedCrossConeLayoutAbiSectionV1 {
    pub fn validate_callables(
        self,
        expected: &crate::CanonicalExactCallableAbiExportsV1,
        meter: &mut BudgetMeter,
    ) -> Result<CallablesResolvedCrossConeLayoutAbiSectionV1, crate::ExactCallableAbiTableError>
    {
        if self.layouts.provider() != expected.provider() {
            return Err(crate::ExactCallableAbiTableError::LayoutProvider);
        }
        if self.layouts.target() != expected.target() {
            return Err(crate::ExactCallableAbiTableError::LayoutTarget);
        }
        Ok(CallablesResolvedCrossConeLayoutAbiSectionV1 {
            layouts: self.layouts,
            descriptors: self.descriptors,
            dispatch: self.dispatch,
            callables: self.callables.validate_against(expected, meter)?,
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
        meter: &mut BudgetMeter,
    ) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError<E>> {
        if self.layouts.provider() != expected.provider()
            || self.layouts.target() != expected.target_profile()
        {
            return Err(LayoutAbiSectionError::LayoutReplayChanged);
        }
        self.validate_callables(expected.callables(), meter)?
            .validate(
                expected,
                dependencies,
                physical_imports,
                source,
                identities,
                meter,
            )
    }
}

impl<D>
    UnselectedCrossConeLayoutAbiSectionV1<
        crate::CanonicalExactLayoutExportsV1,
        crate::CanonicalExactCallableAbiExportsV1,
        D,
    >
{
    pub const fn callables(&self) -> &crate::CanonicalExactCallableAbiExportsV1 {
        &self.callables
    }
}

impl CallablesResolvedCrossConeLayoutAbiSectionV1 {
    pub fn validate_dispatch(
        self,
        expected: &crate::CanonicalExactDispatchExportsV1,
        meter: &mut BudgetMeter,
    ) -> Result<DispatchResolvedCrossConeLayoutAbiSectionV1, crate::ExactDispatchTableError> {
        if self.layouts.provider() != expected.provider() {
            return Err(crate::ExactDispatchTableError::LayoutProvider);
        }
        if self.layouts.target() != expected.target() {
            return Err(crate::ExactDispatchTableError::LayoutTarget);
        }
        Ok(DispatchResolvedCrossConeLayoutAbiSectionV1 {
            layouts: self.layouts,
            descriptors: self.descriptors,
            dispatch: self.dispatch.validate_against(expected, meter)?,
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
        meter: &mut BudgetMeter,
    ) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError<E>> {
        if self.layouts.provider() != expected.provider()
            || self.layouts.target() != expected.target_profile()
        {
            return Err(LayoutAbiSectionError::LayoutReplayChanged);
        }
        self.validate_dispatch(expected.dispatch(), meter)?
            .validate(
                expected,
                dependencies,
                physical_imports,
                source,
                identities,
                meter,
            )
    }
}
