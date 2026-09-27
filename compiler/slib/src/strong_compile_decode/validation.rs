//! Shared foundation and production relation validation for strong profiles.

use super::*;

pub(super) struct StrongProfileLocalProductionSet {
    pub(super) hir: CoreBootstrapInterfaceSectionV1,
    pub(super) mir: CoreBootstrapBridgeSectionV1,
}

pub(crate) struct StrongProfileSemanticFront<'a> {
    pub(crate) hir_foundation: &'a OdrFreeHirFoundation,
    pub(crate) hir_production: &'a CoreBootstrapInterfaceSectionV1,
    pub(crate) mir_production: &'a CoreBootstrapBridgeSectionV1,
    pub(crate) lir_foundation: &'a OdrFreeLirFoundation,
}

pub(crate) fn validate_strong_profile_production(
    graph: &ValidatedGraphArtifact<'_>,
    identities: &mut ValidatedIdentityGraph,
    foundations: &OdrFreeStrongFoundationSet,
    production: DecodedStrongProfileProductionSet,
) -> Result<ValidatedSingleConeStrongProduction, StrongProfileProductionError> {
    let local = validate_strong_profile_local_production(
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
    let lir = validate_strong_profile_lir_production(
        graph,
        identities,
        StrongProfileSemanticFront {
            hir_foundation: &foundations.hir,
            hir_production: &local.hir,
            mir_production: &local.mir,
            lir_foundation: &foundations.lir,
        },
        production.lir,
    )
    .map_err(StrongProfileProductionError::Lir)?;
    Ok(ValidatedSingleConeStrongProduction::new(
        local.hir, local.mir, lir,
    ))
}

pub(super) fn validate_strong_profile_local_production(
    artifact: ConeIdentity,
    identities: &mut ValidatedIdentityGraph,
    hir_foundation: &OdrFreeHirFoundation,
    hir: DecodedCoreBootstrapInterfaceSectionV1,
    mir_foundation: &OdrFreeMirFoundation,
    mir: DecodedCoreBootstrapBridgeSectionV1,
) -> Result<StrongProfileLocalProductionSet, StrongProfileLocalProductionError> {
    let hir = hir
        .validate_against_strong_foundation(artifact, hir_foundation)
        .map_err(StrongProfileLocalProductionError::Hir)?;
    let mir = mir
        .validate_against(artifact, identities, mir_foundation)
        .map_err(StrongProfileLocalProductionError::Mir)?;
    Ok(StrongProfileLocalProductionSet { hir, mir })
}

pub(crate) fn validate_strong_profile_relations(
    provider: ConeIdentity,
    kind: ConeKind,
    hir: &CoreBootstrapInterfaceSectionV1,
    mir: &CoreBootstrapBridgeSectionV1,
    foundation: &OdrFreeHirFoundation,
) -> Result<(), StrongProfileRelationError> {
    validate_output_relation(kind, hir.output_contract(), mir.entry_bridge())?;
    PublicNominalShapeRequirementsV1::from_direct_surface(
        provider,
        hir.direct_public_surface(),
        foundation.as_canonical(),
    )
    .map_err(StrongProfileRelationError::ShapeSources)?;
    validate_protocol_relation(hir.compiler_protocols(), mir.strong_callable_bridges())
}

pub(crate) fn validate_strong_profile_lir_production(
    graph: &ValidatedGraphArtifact<'_>,
    identities: &mut ValidatedIdentityGraph,
    front: StrongProfileSemanticFront<'_>,
    lir: DecodedStrongProductionSectionV1,
) -> Result<StrongProductionSectionV1, StrongProfileLirProductionError> {
    let shape_sources = PublicNominalShapeRequirementsV1::from_direct_surface(
        graph.identity(),
        front.hir_production.direct_public_surface(),
        front.hir_foundation.as_canonical(),
    )
    .and_then(|shapes| shapes.source_declarations(front.hir_foundation.as_canonical()))
    .map_err(StrongProfileLirProductionError::ShapeSources)?;
    validate_strong_profile_lir_with_shape_sources(graph, identities, front, lir, &shape_sources)
}

pub(crate) fn validate_strong_profile_lir_with_shape_sources(
    graph: &ValidatedGraphArtifact<'_>,
    identities: &mut ValidatedIdentityGraph,
    front: StrongProfileSemanticFront<'_>,
    lir: DecodedStrongProductionSectionV1,

    shape_sources: &[scoop_identity::SourceDeclarationKey],
) -> Result<StrongProductionSectionV1, StrongProfileLirProductionError> {
    let entry_source = match front.mir_production.entry_bridge() {
        EntryMirBridgeBranchV1::Library => EntryProductionSourceV1::Library,
        EntryMirBridgeBranchV1::Executable(bridge) => {
            EntryProductionSourceV1::executable(bridge.source().clone())
        }
    };
    let lir = lir
        .validate(
            graph.coordinate().clone(),
            &graph
                .direct_dependencies()
                .iter()
                .map(crate::DependencyRecord::identity)
                .collect::<Vec<_>>(),
            graph.target_selection().target(),
            front.lir_foundation,
            entry_source,
            shape_sources,
            identities,
        )
        .map_err(StrongProfileLirProductionError::Production)?;
    Ok(lir)
}

fn validate_output_relation(
    kind: ConeKind,
    hir: &HirOutputContractV1,
    mir: &EntryMirBridgeBranchV1,
) -> Result<(), StrongProfileRelationError> {
    match (kind, hir, mir) {
        (ConeKind::Library, HirOutputContractV1::Library, EntryMirBridgeBranchV1::Library) => {
            Ok(())
        }
        (
            ConeKind::Executable,
            HirOutputContractV1::Executable(hir),
            EntryMirBridgeBranchV1::Executable(mir),
        ) if hir.as_ref() == mir.source() => Ok(()),
        (ConeKind::Library, _, _) | (ConeKind::Executable, _, _) => {
            Err(StrongProfileRelationError::OutputMismatch)
        }
    }
}

pub(super) fn validate_protocol_relation(
    hir: Option<&CoreCompilerProtocolSurfaceV1>,
    strong: &scoop_mir::StrongCallableBridgeSurfaceV1,
) -> Result<(), StrongProfileRelationError> {
    let actual_cycle = strong.initialization_cycle();
    let (Some(hir), Some(actual_cycle)) = (hir, actual_cycle) else {
        return match (hir, actual_cycle) {
            (None, None) => Ok(()),
            _ => Err(StrongProfileRelationError::InitializationCycleRoleMismatch),
        };
    };

    let cycle = hir.initialization_cycle_thrower();
    let scoop_hir::CoreProtocolCallableDefinitionV1::Function(cycle_definition) =
        cycle.definition()
    else {
        return Err(StrongProfileRelationError::InvalidInitializationCycleDefinition);
    };
    let expected_implementation = scoop_identity::CallableOwner::Function(cycle_definition);
    if actual_cycle.implementation() != expected_implementation {
        return Err(StrongProfileRelationError::InitializationCycleMismatch);
    }
    let expected_signature = scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        None,
        vec![hir.string_exact_type()],
        scoop_mir::core_unit_exact_type(),
    );
    if actual_cycle.signature() != &expected_signature {
        return Err(StrongProfileRelationError::InitializationCycleSignatureMismatch);
    }
    Ok(())
}

pub(crate) fn validate_strong_profile_foundations(
    graph: &mut ValidatedGraphArtifact<'_>,
    identities: &mut ValidatedIdentityGraph,
    hir: DecodedHirFoundation,
    mir: DecodedMirFoundation,
    lir: DecodedLirFoundation,
) -> Result<OdrFreeStrongFoundationSet, StrongProfileFoundationError> {
    validate_strong_profile_foundations_with_source_authority(
        graph,
        identities,
        hir,
        mir,
        lir,
        HirSourceAuthority::CurrentArtifact,
    )
}

pub(crate) fn validate_cross_cone_strong_profile_foundations(
    graph: &mut ValidatedGraphArtifact<'_>,
    identities: &mut ValidatedIdentityGraph,
    hir: DecodedHirFoundation,
    mir: DecodedMirFoundation,
    lir: DecodedLirFoundation,
) -> Result<OdrFreeStrongFoundationSet, StrongProfileFoundationError> {
    validate_strong_profile_foundations_with_source_authority(
        graph,
        identities,
        hir,
        mir,
        lir,
        HirSourceAuthority::DependencyClosure,
    )
}

#[derive(Clone, Copy)]
enum HirSourceAuthority {
    CurrentArtifact,
    DependencyClosure,
}

fn validate_strong_profile_foundations_with_source_authority(
    graph: &mut ValidatedGraphArtifact<'_>,
    identities: &mut ValidatedIdentityGraph,
    hir: DecodedHirFoundation,
    mir: DecodedMirFoundation,
    lir: DecodedLirFoundation,
    source_authority: HirSourceAuthority,
) -> Result<OdrFreeStrongFoundationSet, StrongProfileFoundationError> {
    let coordinate = graph.coordinate().clone();
    let producer = graph.identity();

    let hir = match source_authority {
        HirSourceAuthority::CurrentArtifact => hir.validate(&coordinate, identities),
        HirSourceAuthority::DependencyClosure => {
            hir.validate_with_dependency_sources(&coordinate, identities)
        }
    }
    .map_err(StrongProfileFoundationError::HirStructure)?;
    let mir = mir
        .validate(identities)
        .map_err(StrongProfileFoundationError::MirStructure)?;
    let lir = lir
        .validate(producer, identities)
        .map_err(StrongProfileFoundationError::LirStructure)?;

    Ok(OdrFreeStrongFoundationSet {
        hir: OdrFreeHirFoundation::from_validated(hir)
            .map_err(StrongProfileFoundationError::HirOdr)?,
        mir: OdrFreeMirFoundation::from_validated(mir)
            .map_err(StrongProfileFoundationError::MirOdr)?,
        lir: OdrFreeLirFoundation::from_validated(lir)
            .map_err(StrongProfileFoundationError::LirOdr)?,
    })
}
