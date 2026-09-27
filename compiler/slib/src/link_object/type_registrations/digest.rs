use super::{StrongTypeRegistrationValidationError, TypeRegistrationDigestPlanFailureV1};
use scoop_identity::{DigestNodeKey, PersistentExactTypeId};
use scoop_lir::{DigestFinalizationPlanV1, DigestInputRefV1, StrongTypeRegistrationPlan};

pub(super) fn validate_digest_graph<D: Copy, C>(
    digest_plan: &DigestFinalizationPlanV1,
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
    let mut expected_object_inputs = match plan.definition_owner() {
        scoop_lir::RegistrationDefinitionOwner::Strong => Vec::new(),
        scoop_lir::RegistrationDefinitionOwner::Odr { .. } => vec![
            DigestInputRefV1::ObjectDefinition(plan.descriptor_definition_node()),
            DigestInputRefV1::Layout(plan.layout_fingerprint_node()),
        ],
    };
    expected_object_inputs.sort_unstable();
    if registration_object.direct_inputs() != expected_object_inputs {
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
        .find(|node| *node.key() == plan.definition_owner().digest_key(plan.definition_plan()))
        .ok_or(StrongTypeRegistrationValidationError::DigestPlanMismatch {
            exact_type: plan.exact_type(),
            kind: Failure::MissingRegistrationNode,
        })?;
    if registration.id() != plan.registration_fingerprint_node() {
        return digest_error(plan.exact_type(), Failure::RegistrationNodeIdentity);
    }
    let mut expected_inputs = vec![DigestInputRefV1::from_node(registration_object)];
    match plan.definition_owner() {
        scoop_lir::RegistrationDefinitionOwner::Strong => expected_inputs.extend([
            DigestInputRefV1::from_node(descriptor),
            DigestInputRefV1::from_node(layout),
        ]),
        scoop_lir::RegistrationDefinitionOwner::Odr { .. } => {
            let lir = digest_plan
                .nodes()
                .iter()
                .find(|node| *node.key() == DigestNodeKey::lir_definition(plan.primary_atom()))
                .ok_or(StrongTypeRegistrationValidationError::DigestPlanMismatch {
                    exact_type: plan.exact_type(),
                    kind: Failure::RegistrationDirectInputs,
                })?;
            expected_inputs.push(DigestInputRefV1::from_node(lir));
        }
    }
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
