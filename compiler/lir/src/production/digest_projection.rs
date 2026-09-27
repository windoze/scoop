//! Deterministic digest-DAG projection for the strong production profile.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    CallableBodyKey, DefinitionAtomRole, DigestKind, DigestNodeId, DigestNodeKey,
    DigestPatchIntentKey, DigestSemanticFieldRole, ObjectDefinitionAtomId,
    ObjectDefinitionIdentityError, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PersistentCallableBodyId, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::HashError;

use crate::{
    ConeLirFoundation, DefinitionAtomResolutionError, DigestInputRefV1, DigestNodeBuildError,
    DigestNodeV1, EntryProductionSourceV1, StrongDigestFinalizationPlanV1,
    StrongDigestPlanBuildError, StrongImmortalObjectSemanticPlanSetV1,
    StrongInitializationSchedulePlanV1, StrongInitializationUnitSemanticPlanSet,
    StrongSafepointSemanticPlanSetV1, StrongTypeDescriptorSemanticPlanSet,
};

/// Derives digest inputs from the same runtime semantics used for registrations.
pub(crate) fn project_strong_digest_finalization_plan<D: Copy, C, I>(
    foundation: &ConeLirFoundation,
    entry_source: &EntryProductionSourceV1,
    safepoints: &StrongSafepointSemanticPlanSetV1,
    types: &StrongTypeDescriptorSemanticPlanSet<D, C>,
    immortals: &StrongImmortalObjectSemanticPlanSetV1,
    initialization: &StrongInitializationUnitSemanticPlanSet<I>,
) -> Result<StrongDigestFinalizationPlanV1, StrongDigestProjectionError> {
    DigestGraphWriter::new(foundation).project(
        safepoints,
        types,
        immortals,
        initialization,
        entry_source,
    )
}

mod errors;
pub use errors::StrongDigestProjectionError;
mod graph;
use graph::DigestGraphWriter;
mod replay;
pub use replay::replay_strong_digest_finalization_plan_v2;
