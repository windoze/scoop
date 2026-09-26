use super::*;

/// Source projections and actual local bodies from the same Cone compilation.
#[derive(Clone, Copy)]
pub struct MirTypeBridgeExportInputV1<'a> {
    pub hir: &'a hir::DependencyHirOutput,
    pub public: &'a hir::CrossConeHirInterfaceSectionV1,
    pub source: &'a hir::CrossConeTypeSemanticsSectionV1,
    pub mir: &'a mir::SingleConeStrongMirInput,
    pub ordinary: &'a mir::CrossConeMirBridgeSectionV1,
    pub dependency_objects: &'a [mir::ParamFreeMirObjectValueV1],
    pub identities: &'a ValidatedIdentityGraph,
}

/// Required dependency constituents, without importing or cloning their exports.
#[derive(Clone, Copy)]
pub struct MirTypeBridgeDependencyTablesV1<'a> {
    pub types: &'a [&'a mir::CanonicalParamFreeMirTypeExportsV1],
    pub callables: &'a [&'a mir::CanonicalMirCallableBindingsV1],
    pub direct_callables: &'a [&'a mir::CrossConeMirBridgeSectionV1],
    pub dispatch: &'a [&'a mir::CanonicalMirDispatchSchemasV1],
}

impl MirTypeBridgeExportInputV1<'_> {
    pub(super) fn validate(self) -> Result<(), Error> {
        let provider = self.mir.module().cone;
        if self.hir.output().local.module().cone != provider || self.ordinary.artifact() != provider
        {
            return Err(Error::ProviderMismatch);
        }
        Ok(())
    }
}
