use super::*;

pub(super) fn produce_cross_cone_type_semantics(
    output: &hir::DependencyHirOutput,
    public: &hir::CrossConeHirInterfaceSectionV1,
) -> Result<hir::CrossConeTypeSemanticsProductionV1, hir::CrossConeTypeSemanticsProductionError> {
    produce_type_semantics(output, public)
}

pub(super) fn produce_type_semantics(
    output: &hir::DependencyHirOutput,
    public: &hir::CrossConeHirInterfaceSectionV1,
) -> Result<hir::CrossConeTypeSemanticsProductionV1, hir::CrossConeTypeSemanticsProductionError> {
    with_metadata(output, public, |metadata, dependencies| {
        crate::produce_cross_cone_type_semantics(output, metadata, dependencies)
    })
}

pub(super) fn with_metadata<R>(
    output: &hir::DependencyHirOutput,
    public: &hir::CrossConeHirInterfaceSectionV1,
    run: impl FnOnce(hir::SharedTypeMetadataV1<'_>, &[hir::SharedTypeMetadataV1<'_>]) -> R,
) -> R {
    let core = trusted_core();
    let identities = source_inventory::identity_closure(output);
    let foundation = hir::OdrFreeHirFoundation::try_new(
        hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap(),
    )
    .unwrap();
    let metadata = hir::SharedTypeMetadataV1 {
        provider: output.output().export.cone,
        identities: &identities,
        foundation: &foundation,
        public,
    };
    let dependency = hir::SharedTypeMetadataV1 {
        provider: ConeIdentity::CORE,
        identities: &identities,
        foundation: &core.source_foundation,
        public: core.general_interface(),
    };
    run(metadata, &[dependency])
}
