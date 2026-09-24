//! Deterministic digest-DAG projection for the strong production profile.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    CallableBodyKey, DefinitionAtomRole, DigestKind, DigestNodeId, DigestNodeKey,
    DigestPatchIntentKey, DigestSemanticFieldRole, ObjectDefinitionAtomId,
    ObjectDefinitionIdentityError, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PersistentCallableBodyId, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{BudgetMeter, HashError};

use crate::{
    DefinitionAtomResolutionError, DigestInputRefV1, DigestNodeBuildError, DigestNodeV1,
    EntryProductionSourceV1, Module, OdrFreeLirFoundation, StrongDigestFinalizationPlanV1,
    StrongDigestPlanBuildError, StrongImmortalObjectSemanticPlanBuildError,
    StrongImmortalObjectSemanticPlanSetV1, StrongInitializationSchedulePlanV1,
    StrongInitializationUnitSemanticPlanBuildError, StrongInitializationUnitSemanticPlanSet,
    StrongInitializationUnitSemanticPlanSetV1, StrongSafepointSemanticPlanError,
    StrongSafepointSemanticPlanSetV1, StrongTypeDescriptorSemanticPlanBuildError,
    StrongTypeDescriptorSemanticPlanSet, StrongTypeDescriptorSemanticPlanSetV1,
};

/// Projects the only digest graph accepted by the single-Cone strong writer.
///
/// This function deliberately derives every node, edge, and patch from the
/// final LIR graph and its sealed foundation. Callers cannot supply a partial
/// graph or add an alternative digest path.
pub(crate) fn project_strong_digest_finalization_plan(
    module: &Module,
    foundation: &OdrFreeLirFoundation,
    entry_source: &EntryProductionSourceV1,
) -> Result<StrongDigestFinalizationPlanV1, StrongDigestProjectionError> {
    if module.cone != foundation.producer() {
        return Err(StrongDigestProjectionError::ProducerMismatch {
            module: module.cone,
            foundation: foundation.producer(),
        });
    }

    let safepoints = StrongSafepointSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::Safepoints)?;
    let types = StrongTypeDescriptorSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::Types)?;
    let immortals = StrongImmortalObjectSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::ImmortalObjects)?;
    let initialization = StrongInitializationUnitSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::InitializationUnits)?;

    DigestGraphWriter::new(foundation).project(
        &safepoints,
        &types,
        &immortals,
        &initialization,
        entry_source,
    )
}

/// Projects the same digest graph while validating the V2 descriptor
/// semantics. Initialization dependency payloads do not contribute digest
/// inputs, so their local semantic base can be used before external unit
/// definitions are joined to the completed digest identities.
pub(crate) fn project_strong_digest_finalization_plan_v2(
    module: &Module,
    foundation: &OdrFreeLirFoundation,
    entry_source: &EntryProductionSourceV1,
    selected: &crate::StrongProductionDependencySelectionV2<'_>,
    meter: &mut BudgetMeter,
) -> Result<StrongDigestFinalizationPlanV1, StrongDigestProjectionError> {
    if module.cone != foundation.producer() {
        return Err(StrongDigestProjectionError::ProducerMismatch {
            module: module.cone,
            foundation: foundation.producer(),
        });
    }
    let safepoints = StrongSafepointSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::Safepoints)?;
    let types = crate::StrongTypeDescriptorSemanticPlanSetV2::from_module(module, selected, meter)
        .map_err(StrongDigestProjectionError::Types)?;
    let immortals = StrongImmortalObjectSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::ImmortalObjects)?;
    let initialization = StrongInitializationUnitSemanticPlanSetV1::from_module(module)
        .map_err(StrongDigestProjectionError::InitializationUnits)?;
    replay::charge_projection(
        foundation,
        [
            foundation.callable_bodies().len(),
            types.descriptors().len(),
            safepoints.sites().len(),
            immortals.objects().len(),
            initialization.static_storages().storages().len(),
            initialization.units().len(),
        ],
        meter,
    )?;
    DigestGraphWriter::new(foundation).project(
        &safepoints,
        &types,
        &immortals,
        &initialization,
        entry_source,
    )
}

mod errors;
pub use errors::StrongDigestProjectionError;
mod graph;
use graph::DigestGraphWriter;
mod replay;
pub use replay::replay_strong_digest_finalization_plan_v2;
