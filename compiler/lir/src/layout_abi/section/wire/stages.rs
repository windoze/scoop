use super::*;
use scoop_wire::encode_canonical_temporary_with_meter;

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

impl<C> UnselectedCrossConeLayoutAbiSectionV1<crate::CanonicalExactLayoutExportsV1, C> {
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

impl CallablesResolvedCrossConeLayoutAbiSectionV1 {
    pub const fn callables(&self) -> &crate::CanonicalExactCallableAbiExportsV1 {
        &self.callables
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
            || !same_bytes(&self.layouts, expected.layouts(), meter)?
        {
            return Err(LayoutAbiSectionError::LayoutReplayChanged);
        }
        if !same_bytes(&self.callables, expected.callables(), meter)? {
            return Err(LayoutAbiSectionError::CallableReplayChanged);
        }
        let dependencies = dependencies::complete(
            expected.provider(),
            expected.target_profile(),
            dependencies,
            meter,
        )?;
        let physical_imports =
            crate::CanonicalExternalShapeLinkImportsV1::from_checked(physical_imports, meter)?;
        let descriptors = self
            .descriptors
            .validate_against(expected.descriptors(), meter)?;
        let dispatch = self.dispatch.validate_against(expected.dispatch(), meter)?;
        if !same_bytes(&self.shape_support, expected.shape_support(), meter)? {
            return Err(LayoutAbiSectionError::ShapeSupport);
        }
        let exports = LayoutAbiExportConstituentsV1::try_new(
            self.layouts,
            descriptors,
            dispatch,
            self.callables,
            expected.shape_support().clone(),
        )?;
        let mut semantic = reserve(self.selected.semantic.len(), meter)?;
        for relation in self.selected.semantic {
            semantic.push(relation.resolve(identities, meter)?);
        }
        let physical_imports = self
            .selected
            .physical
            .validate_against(&physical_imports, meter)?;
        build::complete(
            exports,
            dependencies,
            physical_imports,
            build::SelectionInput::Reader(semantic),
            source,
            meter,
        )
    }
}

fn same_bytes(
    actual: &impl WireEncode,
    expected: &impl WireEncode,
    meter: &mut BudgetMeter,
) -> Result<bool, WireError> {
    let path = WirePath::root();
    let actual = encode_canonical_temporary_with_meter(actual, meter, &path)?;
    let expected = encode_canonical_temporary_with_meter(expected, meter, &path)?;
    meter.charge_work(actual.len() as u64, &path)?;
    Ok(actual == expected)
}
