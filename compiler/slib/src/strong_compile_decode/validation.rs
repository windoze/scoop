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
    expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
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
    validate_strong_profile_relations(graph.kind(), &local.hir, &local.mir, &foundations.hir)
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
        expected_external_bridges,
    )
    .map_err(StrongProfileProductionError::Lir)?;
    Ok(ValidatedSingleConeStrongProduction {
        hir: local.hir,
        mir: local.mir,
        lir,
    })
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
        .validate_against_strong_foundation(artifact, identities, mir_foundation)
        .map_err(StrongProfileLocalProductionError::Mir)?;
    Ok(StrongProfileLocalProductionSet { hir, mir })
}

pub(crate) fn validate_strong_profile_relations(
    kind: ConeKind,
    hir: &CoreBootstrapInterfaceSectionV1,
    mir: &CoreBootstrapBridgeSectionV1,
    foundation: &OdrFreeHirFoundation,
) -> Result<(), StrongProfileRelationError> {
    validate_output_relation(kind, hir.output_contract(), mir.entry_bridge())?;
    PublicNominalShapeRequirementsV1::from_direct_surface(
        hir.direct_public_surface(),
        foundation.as_canonical(),
    )
    .map_err(StrongProfileRelationError::ShapeSources)?;
    validate_core_relation(
        hir.core_interface(),
        mir.core_bridge(),
        mir.strong_callable_bridges(),
    )
}

pub(crate) fn validate_strong_profile_lir_production(
    graph: &ValidatedGraphArtifact<'_>,
    identities: &mut ValidatedIdentityGraph,
    front: StrongProfileSemanticFront<'_>,
    lir: DecodedStrongProductionSectionV1,
    expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
) -> Result<StrongProductionSectionV1, StrongProfileLirProductionError> {
    let entry_source = match front.mir_production.entry_bridge() {
        EntryMirBridgeBranchV1::Library => EntryProductionSourceV1::Library,
        EntryMirBridgeBranchV1::Executable(bridge) => {
            EntryProductionSourceV1::executable(bridge.source().clone())
        }
    };
    let core_shape_sources = match front.hir_production.core_interface() {
        CoreHirInterfaceBranchV1::NotCore => Vec::new(),
        CoreHirInterfaceBranchV1::Core(_) => PublicNominalShapeRequirementsV1::from_direct_surface(
            front.hir_production.direct_public_surface(),
            front.hir_foundation.as_canonical(),
        )
        .and_then(|shapes| shapes.source_declarations(front.hir_foundation.as_canonical()))
        .map_err(StrongProfileLirProductionError::ShapeSources)?,
    };
    let lir = lir
        .validate(
            graph.coordinate().clone(),
            graph.target_selection().target(),
            front.lir_foundation,
            expected_external_bridges,
            entry_source,
            &core_shape_sources,
            identities,
        )
        .map_err(StrongProfileLirProductionError::Production)?;
    validate_core_lir_relation(
        front.mir_production.core_bridge(),
        front.mir_production.strong_callable_bridges(),
        lir.core_lir_bridge(),
    )
    .map_err(StrongProfileLirProductionError::CoreRelation)?;
    Ok(lir)
}

fn validate_core_lir_relation(
    mir: &CoreMirBridgeBranchV1,
    strong: &scoop_mir::StrongCallableBridgeSurfaceV1,
    lir: &CoreLirBridgeBranchV1,
) -> Result<(), StrongProfileCoreLirRelationError> {
    let (CoreMirBridgeBranchV1::Core(mir), CoreLirBridgeBranchV1::Core(lir)) = (mir, lir) else {
        return match (mir, lir) {
            (CoreMirBridgeBranchV1::NotCore, CoreLirBridgeBranchV1::NotCore) => Ok(()),
            _ => Err(StrongProfileCoreLirRelationError::BranchMismatch),
        };
    };
    let mir_cycle = mir.initialization_cycle_thrower();
    let cycle_implementation = mir_cycle.implementation();
    let cycle_target = match cycle_implementation {
        scoop_identity::CallableOwner::Function(id) => {
            scoop_identity::StrongCallableDefinitionOwner::Function(id)
        }
        _ => return Err(StrongProfileCoreLirRelationError::InvalidInitializationCycleOwner),
    };
    let lir_cycle = lir.initialization_cycle_thrower();
    if lir_cycle.target() != cycle_target {
        return Err(StrongProfileCoreLirRelationError::InitializationCycleMismatch);
    }
    let cycle_exact = strong
        .bridges()
        .iter()
        .find(|bridge| bridge.implementation() == cycle_implementation)
        .ok_or(StrongProfileCoreLirRelationError::MissingInitializationCycleSignature)?;
    if lir_cycle.abi_signature().signature() != cycle_exact.signature() {
        return Err(StrongProfileCoreLirRelationError::InitializationCycleSignatureMismatch);
    }
    Ok(())
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

pub(super) fn validate_core_relation(
    hir: &CoreHirInterfaceBranchV1,
    mir: &CoreMirBridgeBranchV1,
    strong: &scoop_mir::StrongCallableBridgeSurfaceV1,
) -> Result<(), StrongProfileRelationError> {
    let (CoreHirInterfaceBranchV1::Core(hir), CoreMirBridgeBranchV1::Core(mir)) = (hir, mir) else {
        return match (hir, mir) {
            (CoreHirInterfaceBranchV1::NotCore, CoreMirBridgeBranchV1::NotCore) => Ok(()),
            _ => Err(StrongProfileRelationError::CoreBranchMismatch),
        };
    };

    let cycle = hir.compiler_protocols().initialization_cycle_thrower();
    let scoop_hir::CoreProtocolCallableDefinitionV1::Function(cycle_definition) =
        cycle.definition()
    else {
        return Err(StrongProfileRelationError::InvalidInitializationCycleDefinition);
    };
    let actual_cycle = mir.initialization_cycle_thrower();
    let expected_implementation = scoop_identity::CallableOwner::Function(cycle_definition);
    if actual_cycle.definition() != cycle_definition
        || actual_cycle.implementation() != expected_implementation
    {
        return Err(StrongProfileRelationError::InitializationCycleMismatch);
    }
    let expected_signature = scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        None,
        vec![hir.string_capability().exact_type()],
        scoop_mir::core_unit_exact_type(),
    );
    let actual_signature = strong
        .bridges()
        .iter()
        .find(|bridge| bridge.implementation() == expected_implementation)
        .ok_or(StrongProfileRelationError::MissingInitializationCycleSignature)?;
    if actual_signature.signature() != &expected_signature {
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
    let meter = graph.envelope.meter_mut();
    let hir = match source_authority {
        HirSourceAuthority::CurrentArtifact => hir.validate(&coordinate, identities, meter),
        HirSourceAuthority::DependencyClosure => {
            hir.validate_with_dependency_sources(&coordinate, identities, meter)
        }
    }
    .map_err(StrongProfileFoundationError::HirStructure)?;
    let mir = mir
        .validate(identities, meter)
        .map_err(StrongProfileFoundationError::MirStructure)?;
    let lir = lir
        .validate(producer, identities, meter)
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
