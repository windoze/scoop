use super::{StrongTypeRegistrationValidationError, TypeRegistrationDigestPlanFailureV1};
use scoop_identity::{DigestNodeKey, PersistentExactTypeId};
use scoop_lir::{DigestInputRefV1, StrongDigestFinalizationPlanV1, StrongTypeRegistrationPlan};

pub(super) fn validate_digest_graph<D: Copy, C>(
    digest_plan: &StrongDigestFinalizationPlanV1,
    plan: &StrongTypeRegistrationPlan<D, C>,
) -> Result<(), StrongTypeRegistrationValidationError> {
    use TypeRegistrationDigestPlanFailureV1 as Failure;

    let registration_object = digest_plan
        .nodes()
        .iter()
        .find(|node| node.key() == &DigestNodeKey::object_definition(plan.primary_atom()))
        .ok_or(StrongTypeRegistrationValidationError::DigestPlanMismatch {
            exact_type: plan.exact_type(),
            kind: Failure::MissingRegistrationObjectDefinitionNode,
        })?;
    if registration_object.id() != plan.registration_object_node() {
        return digest_error(
            plan.exact_type(),
            Failure::RegistrationObjectDefinitionNodeIdentity,
        );
    }
    if !registration_object.direct_inputs().is_empty() {
        return digest_error(
            plan.exact_type(),
            Failure::RegistrationObjectDefinitionDirectInputs,
        );
    }
    if !registration_object.patch_intents().is_empty() {
        return digest_error(
            plan.exact_type(),
            Failure::RegistrationObjectDefinitionPatchSet,
        );
    }

    let descriptor = digest_plan
        .nodes()
        .iter()
        .find(|node| {
            node.key() == &DigestNodeKey::object_definition(plan.descriptor_primary_atom())
        })
        .ok_or(StrongTypeRegistrationValidationError::DigestPlanMismatch {
            exact_type: plan.exact_type(),
            kind: Failure::MissingDescriptorObjectDefinitionNode,
        })?;
    if descriptor.id() != plan.descriptor_definition_node() {
        return digest_error(
            plan.exact_type(),
            Failure::DescriptorObjectDefinitionNodeIdentity,
        );
    }

    let layout = digest_plan
        .nodes()
        .iter()
        .find(|node| node.key() == &DigestNodeKey::layout(plan.layout()))
        .ok_or(StrongTypeRegistrationValidationError::DigestPlanMismatch {
            exact_type: plan.exact_type(),
            kind: Failure::MissingLayoutNode,
        })?;
    if layout.id() != plan.layout_fingerprint_node() {
        return digest_error(plan.exact_type(), Failure::LayoutNodeIdentity);
    }

    let registration = digest_plan
        .nodes()
        .iter()
        .find(|node| node.key() == &DigestNodeKey::strong_registration(plan.definition_plan()))
        .ok_or(StrongTypeRegistrationValidationError::DigestPlanMismatch {
            exact_type: plan.exact_type(),
            kind: Failure::MissingRegistrationNode,
        })?;
    if registration.id() != plan.registration_fingerprint_node() {
        return digest_error(plan.exact_type(), Failure::RegistrationNodeIdentity);
    }
    let mut expected_inputs = vec![
        DigestInputRefV1::from_node(registration_object),
        DigestInputRefV1::from_node(descriptor),
        DigestInputRefV1::from_node(layout),
    ];
    expected_inputs.sort_unstable();
    if registration.direct_inputs() != expected_inputs {
        return digest_error(plan.exact_type(), Failure::RegistrationDirectInputs);
    }
    if registration.patch_intents().len() != 1
        || registration.patch_intents()[0].id() != plan.registration_definition_patch()
    {
        return digest_error(plan.exact_type(), Failure::RegistrationPatchSet);
    }
    if !descriptor
        .patch_intents()
        .iter()
        .any(|patch| patch.id() == plan.descriptor_definition_patch())
    {
        return digest_error(plan.exact_type(), Failure::DescriptorDefinitionPatchMissing);
    }
    if !layout
        .patch_intents()
        .iter()
        .any(|patch| patch.id() == plan.layout_fingerprint_patch())
    {
        return digest_error(plan.exact_type(), Failure::LayoutPatchMissing);
    }
    Ok(())
}

fn digest_error<T>(
    exact_type: PersistentExactTypeId,
    kind: TypeRegistrationDigestPlanFailureV1,
) -> Result<T, StrongTypeRegistrationValidationError> {
    Err(StrongTypeRegistrationValidationError::DigestPlanMismatch { exact_type, kind })
}
