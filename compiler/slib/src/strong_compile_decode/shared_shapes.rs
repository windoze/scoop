use super::*;
use scoop_hir::CrossConeHirInterfaceSectionV1;

pub(crate) fn validate_shared_strong_profile_production(
    graph: &mut ValidatedGraphArtifact<'_>,
    identities: &mut ValidatedIdentityGraph,
    foundations: &OdrFreeStrongFoundationSet,
    production: DecodedStrongProfileProductionSet,
    interface: &CrossConeHirInterfaceSectionV1,
    expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
) -> Result<ValidatedSingleConeStrongProduction, StrongProfileProductionError> {
    let local = validation::validate_strong_profile_local_production(
        graph.identity(),
        identities,
        &foundations.hir,
        production.hir,
        &foundations.mir,
        production.mir,
    )
    .map_err(StrongProfileProductionError::Local)?;
    validate_strong_profile_relations(
        graph.identity(),
        graph.kind(),
        &local.hir,
        &local.mir,
        &foundations.hir,
    )
    .map_err(StrongProfileProductionError::Relation)?;
    let sources = PublicNominalShapeRequirementsV1::from_shared_surface(
        graph.identity(),
        local.hir.direct_public_surface(),
        foundations.hir.as_canonical(),
        interface.nominal_interfaces(),
        interface.callable_interfaces(),
        graph.envelope.meter_mut(),
    )
    .and_then(|roots| roots.source_declarations(foundations.hir.as_canonical()))
    .map_err(StrongProfileLirProductionError::ShapeSources)
    .map_err(StrongProfileProductionError::Lir)?;
    let lir = validate_strong_profile_lir_with_shape_sources(
        graph,
        identities,
        StrongProfileSemanticFront {
            hir_foundation: &foundations.hir,
            hir_production: &local.hir,
            mir_production: &local.mir,
            lir_foundation: &foundations.lir,
        },
        production.lir,
        expected_external_bridges,
        &sources,
    )
    .map_err(StrongProfileProductionError::Lir)?;
    Ok(ValidatedSingleConeStrongProduction {
        hir: local.hir,
        mir: local.mir,
        lir,
    })
}
