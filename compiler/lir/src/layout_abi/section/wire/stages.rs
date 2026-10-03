use super::*;

impl DecodedCrossConeLayoutAbiSectionV1 {
    pub fn validate_layouts(
        self,
        expected: &crate::CanonicalExactLayoutExportsV1,
    ) -> Result<LayoutsResolvedCrossConeLayoutAbiSectionV1, crate::ExactLayoutTableError> {
        Ok(LayoutsResolvedCrossConeLayoutAbiSectionV1 {
            layouts: self.layouts.validate_against(expected)?,
            descriptors: self.descriptors,
            dispatch: self.dispatch,
            callables: self.callables,
            shape_support: self.shape_support,
            selected: self.selected,
        })
    }
}

impl<C, D, T> UnselectedCrossConeLayoutAbiSectionV1<crate::CanonicalExactLayoutExportsV1, C, D, T> {
    pub const fn layouts(&self) -> &crate::CanonicalExactLayoutExportsV1 {
        &self.layouts
    }
}

impl LayoutsResolvedCrossConeLayoutAbiSectionV1 {
    pub fn validate_callables(
        self,
        expected: &crate::CanonicalExactCallableAbiExportsV1,
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
            callables: self.callables.validate_against(expected)?,
            shape_support: self.shape_support,
            selected: self.selected,
        })
    }
}

impl<D, T>
    UnselectedCrossConeLayoutAbiSectionV1<
        crate::CanonicalExactLayoutExportsV1,
        crate::CanonicalExactCallableAbiExportsV1,
        D,
        T,
    >
{
    pub const fn callables(&self) -> &crate::CanonicalExactCallableAbiExportsV1 {
        &self.callables
    }
}

impl CallablesResolvedCrossConeLayoutAbiSectionV1 {
    pub const fn dispatch_wire(&self) -> &crate::DecodedCanonicalExactDispatchExportsV1 {
        &self.dispatch
    }

    pub fn validate_dispatch(
        self,
        expected: &crate::CanonicalExactDispatchExportsV1,
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
            dispatch: self.dispatch.validate_against(expected)?,
            callables: self.callables,
            shape_support: self.shape_support,
            selected: self.selected,
        })
    }
}
