use super::*;
use crate::{ExternalStrongShapeSubjectV1, LinkDataError, link_data::link_error};
use scoop_identity::{
    CallableBodyKey, LinkageClass, ObjectDefinitionPlanOwner, ObjectDefinitionPlanRole,
    PersistentSymbolKey, StrongDefinitionEntity, StrongDefinitionRole, ValidatedIdentityGraph,
};

pub(super) fn read(
    owner: CallableDefinitionOwner,
    definition: StrongShapeDefinitionV1<PersistentCallableBodyId>,
    foundation: &ConeLirFoundation,
    identities: &ValidatedIdentityGraph,
) -> Result<StrongShapeDefinitionRefV1, LinkDataError> {
    let body = definition.semantic_id();
    let body_key = identities
        .canonical_key::<_, CallableBodyKey>(body)
        .map_err(link_error)?;
    if body_key.as_ref() != &owner.body_key() || !foundation.contains_callable_body(body) {
        return Err(link_error(
            "callable ABI refers to a different or missing body",
        ));
    }
    let plan = foundation
        .definition_plan(definition.definition_plan())
        .ok_or_else(|| link_error("missing callable definition plan"))?;
    let (expected_owner, expected_role, linkage) = match owner {
        CallableDefinitionOwner::Strong(_) => (
            ObjectDefinitionPlanOwner::Strong {
                producer: foundation.producer(),
                entity: StrongDefinitionEntity::callable_body(body),
            },
            ObjectDefinitionPlanRole::Strong(StrongDefinitionRole::CallableBody),
            LinkageClass::ConeStrong,
        ),
        CallableDefinitionOwner::Odr(member) => (
            ObjectDefinitionPlanOwner::Odr {
                member: member.member(),
            },
            ObjectDefinitionPlanRole::OdrMemberPrimary,
            LinkageClass::OdrWeak,
        ),
    };
    let symbol = definition.symbol();
    if plan.key().owner() != expected_owner
        || plan.key().definition_role() != expected_role
        || symbol.key() != PersistentSymbolKey::CallableBody(body)
        || symbol.linkage() != linkage
    {
        return Err(link_error(
            "callable ABI definition does not refer to its body",
        ));
    }
    StrongShapeDefinitionRefV1::from_saved(
        ExternalStrongShapeSubjectV1::Callable(owner),
        definition.definition_plan(),
        symbol,
        foundation,
    )
}
