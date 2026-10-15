use super::*;

impl DecodedCrossConeLayoutAbiSectionV1 {
    pub fn read_layouts(
        self,
        target: crate::LirTargetProfile,
        foundation: &crate::ConeLirFoundation,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<LayoutsResolvedCrossConeLayoutAbiSectionV1, crate::LinkDataError> {
        Ok(LayoutsResolvedCrossConeLayoutAbiSectionV1 {
            layouts: self.layouts.read(target, foundation, identities)?,
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
    pub fn read_callables(
        self,
        target: crate::LirTargetProfile,
        foundation: &crate::ConeLirFoundation,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<CallablesResolvedCrossConeLayoutAbiSectionV1, crate::LinkDataError> {
        if self.layouts.provider() != foundation.producer() || self.layouts.target() != target {
            return Err(crate::LinkDataError(
                "callable table layout provider or target mismatch".into(),
            ));
        }
        Ok(CallablesResolvedCrossConeLayoutAbiSectionV1 {
            layouts: self.layouts,
            descriptors: self.descriptors,
            dispatch: self.dispatch,
            callables: self.callables.read(target, foundation, identities)?,
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
    pub fn read_dispatch(
        self,
        target: crate::LirTargetProfile,
        foundation: &crate::ConeLirFoundation,
        dependencies: &[&crate::CanonicalExactLayoutExportsV1],
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<DispatchResolvedCrossConeLayoutAbiSectionV1, crate::LinkDataError> {
        if self.layouts.provider() != foundation.producer() || self.layouts.target() != target {
            return Err(crate::LinkDataError(
                "dispatch layout provider or target mismatch".into(),
            ));
        }
        let layouts = std::iter::once(&self.layouts)
            .chain(dependencies.iter().copied())
            .collect::<Vec<_>>();
        let dispatch = self
            .dispatch
            .read(target, foundation, &layouts, identities)?;
        Ok(DispatchResolvedCrossConeLayoutAbiSectionV1 {
            layouts: self.layouts,
            descriptors: self.descriptors,
            callables: self.callables,
            dispatch,
            shape_support: self.shape_support,
            selected: self.selected,
        })
    }
}
