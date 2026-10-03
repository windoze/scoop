//! Deterministic digest-DAG projection for the strong production profile.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    CallableBodyKey, DefinitionAtomRole, DigestKind, DigestNodeId, DigestNodeKey,
    DigestPatchIntentKey, DigestSemanticFieldRole, ObjectDefinitionAtomId,
    ObjectDefinitionIdentityError, ObjectDefinitionPlanId, PersistentCallableBodyId,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::HashError;

use crate::{
    ConeLirFoundation, DefinitionAtomResolutionError, DigestFinalizationPlanV1, DigestInputRefV1,
    DigestNodeBuildError, DigestNodeV1, DigestPlanBuildError, EntryProductionSourceV1,
    StrongImmortalObjectSemanticPlanSetV1, StrongInitializationSchedulePlanV1,
    StrongInitializationUnitSemanticPlanSet, StrongSafepointSemanticPlanSetV1,
    StrongTypeDescriptorSemanticPlanSet,
};

/// Derives digest inputs from the same runtime semantics used for registrations.
pub(crate) fn project_digest_finalization_plan<D: Copy, C, I>(
    foundation: &ConeLirFoundation,
    entry_source: &EntryProductionSourceV1,
    safepoints: &StrongSafepointSemanticPlanSetV1,
    types: &StrongTypeDescriptorSemanticPlanSet<D, C>,
    immortals: &StrongImmortalObjectSemanticPlanSetV1,
    initialization: &StrongInitializationUnitSemanticPlanSet<I>,
) -> Result<DigestFinalizationPlanV1, DigestProjectionError> {
    DigestGraphWriter::new(foundation).project(
        safepoints,
        types,
        immortals,
        initialization,
        entry_source,
    )
}

mod errors;
pub use errors::DigestProjectionError;
mod graph;
use graph::DigestGraphWriter;
mod replay;
pub use replay::replay_digest_finalization_plan_v2;
