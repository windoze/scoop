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
    pub(super) fn validate(self) -> Result<(), Error> {
        use hir::NominalRepresentationSemanticAuthority;
        let provider = self.mir.module().cone;
        if self.hir.output().local.module().cone != provider
            || self.source.foundation().current_provider() != provider
            || self.ordinary.artifact() != provider
        {
            return Err(Error::ProviderMismatch);
        }
        super::callables::validate_ordinary(self)?;
        Ok(())
    }
}
