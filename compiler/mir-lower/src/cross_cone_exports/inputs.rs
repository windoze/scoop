use super::*;

/// Source projections and actual local bodies from the same Cone compilation.
#[derive(Clone, Copy)]
pub struct MirTypeBridgeExportInputV1<'a> {
    pub hir: &'a hir::DependencyHirOutput,
    pub public: &'a hir::CrossConeHirInterfaceSectionV1,
    pub source: &'a hir::CrossConeTypeSemanticsProductionV1,
    pub mir: &'a mir::SingleConeStrongMirInput,
    pub ordinary: &'a mir::CrossConeMirBridgeSectionV1,
    pub nominal_classifier: &'a hir::NominalExactLeafClassifierV1,
    pub identities: &'a ValidatedIdentityGraph,
}

/// Required dependency constituents, without importing or cloning their exports.
#[derive(Clone, Copy)]
pub struct MirTypeBridgeDependencyTablesV1<'a> {
    pub types: &'a [&'a mir::CanonicalParamFreeMirTypeExportsV1],
    pub callables: &'a [&'a mir::CanonicalMirCallableBindingsV1],
    pub dispatch: &'a [&'a mir::CanonicalMirDispatchSchemasV1],
}

impl MirTypeBridgeExportInputV1<'_> {
    pub(super) fn validate(
        self,
        uses: &mir::CanonicalMirExternalInitializationUsesV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        use hir::NominalRepresentationSemanticAuthority;
        let provider = self.mir.module().cone;
        if self.hir.output().local.module().cone != provider
            || self.source.foundation().current_provider() != provider
            || self.ordinary.artifact() != provider
        {
            return Err(Error::ProviderMismatch);
        }
        super::callables::validate_ordinary(self, meter)?;
        let roots = self.mir.materialization().initialization_roots();
        let mut units = reserve(roots.len(), meter)?;
        units.extend(roots.iter().map(|root| root.identity()));
        meter.charge_work(
            (units.len() as u64)
                .saturating_mul(u64::from(units.len().checked_ilog2().unwrap_or(0)) + 1),
            &WirePath::root(),
        )?;
        units.sort_unstable();
        for usage in uses.records() {
            meter.charge_work(
                u64::from(units.len().checked_ilog2().unwrap_or(0)) + 1,
                &WirePath::root(),
            )?;
            if units.binary_search(&usage.local_unit()).is_err() {
                return Err(Error::MissingInitializationUnit(usage.local_unit()));
            }
            mir::SelectedExternalInitializationUseV1::try_new(
                provider,
                self.identities,
                usage.local_unit(),
                usage.provider(),
                usage.dependency_unit(),
                usage.cause(),
                meter,
            )
            .map_err(Error::InitializationUse)?;
        }
        Ok(())
    }
}
