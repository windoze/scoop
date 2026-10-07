//! Reuse the artifact's typed binding for local and cross-object TD references.

use scoop_identity::ObjectDefinitionPlanId;

use crate::SlibMemberId;
use crate::link_object::{
    StrongRelocationResolutionV1, VerifiedBuiltinObjectStrongRelocationSetV1,
    VerifiedRelocationUseV1,
};

pub(super) fn local_definition(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    member: SlibMemberId,
    relocation: &VerifiedRelocationUseV1,
) -> Option<ObjectDefinitionPlanId> {
    let [binding] = builtins.strong_relocations().bindings_at(
        member,
        relocation.containing_atom(),
        relocation.offset_within_atom(),
    ) else {
        return None;
    };
    match binding.resolution() {
        StrongRelocationResolutionV1::ObjectLocalStrong { definition, .. }
        | StrongRelocationResolutionV1::CurrentConeUndefinedStrong { definition, .. } => {
            Some(definition)
        }
        StrongRelocationResolutionV1::ExternalCandidate { .. } => None,
    }
}
