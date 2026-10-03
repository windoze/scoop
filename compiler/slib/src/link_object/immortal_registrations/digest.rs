use scoop_identity::{DigestNodeKey, PersistentImmortalObjectId};
use scoop_lir::{
    DigestFinalizationPlanV1, DigestInputRefV1, StrongImmortalObjectRegistrationPlanV1,
};

use super::{
    ImmortalObjectRegistrationDigestPlanFailureV1, StrongImmortalObjectRegistrationValidationError,
};

pub(super) fn validate_digest_graph(
    digest_plan: &DigestFinalizationPlanV1,
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> Result<(), StrongImmortalObjectRegistrationValidationError> {
    use ImmortalObjectRegistrationDigestPlanFailureV1 as Failure;

    let registration_object = digest_plan
        .nodes()
        .iter()
        .find(|node| {
            node.key() == &DigestNodeKey::object_definition(plan.registration_primary_atom())
        })
        .ok_or(
            StrongImmortalObjectRegistrationValidationError::DigestPlanMismatch {
                object: plan.object(),
                kind: Failure::MissingRegistrationObjectDefinitionNode,
            },
        )?;
    if registration_object.id() != plan.registration_object_node() {
        return digest_error(
            plan.object(),
            Failure::RegistrationObjectDefinitionNodeIdentity,
        );
    }
    if !registration_object.direct_inputs().is_empty() {
        return digest_error(
            plan.object(),
            Failure::RegistrationObjectDefinitionDirectInputs,
        );
    }
    if !registration_object.patch_intents().is_empty() {
        return digest_error(plan.object(), Failure::RegistrationObjectDefinitionPatchSet);
    }

    let immortal_object = digest_plan
        .nodes()
        .iter()
        .find(|node| node.key() == &DigestNodeKey::object_definition(plan.object_primary_atom()))
        .ok_or(
            StrongImmortalObjectRegistrationValidationError::DigestPlanMismatch {
                object: plan.object(),
                kind: Failure::MissingImmortalObjectDefinitionNode,
            },
        )?;
    if immortal_object.id() != plan.object_definition_node() {
        return digest_error(plan.object(), Failure::ImmortalObjectDefinitionNodeIdentity);
    }
    if !immortal_object.direct_inputs().is_empty() {
        return digest_error(plan.object(), Failure::ImmortalObjectDefinitionDirectInputs);
    }
    if !immortal_object.patch_intents().is_empty() {
        return digest_error(plan.object(), Failure::ImmortalObjectDefinitionPatchSet);
    }

    let registration = digest_plan
        .nodes()
        .iter()
        .find(|node| {
            node.key()
                == &plan
                    .definition_owner()
                    .digest_key(plan.registration_definition_plan())
        })
        .ok_or(
            StrongImmortalObjectRegistrationValidationError::DigestPlanMismatch {
                object: plan.object(),
                kind: Failure::MissingRegistrationNode,
            },
        )?;
    if registration.id() != plan.registration_fingerprint_node() {
        return digest_error(plan.object(), Failure::RegistrationNodeIdentity);
    }
    let mut expected_inputs = vec![
        DigestInputRefV1::from_node(registration_object),
        DigestInputRefV1::from_node(immortal_object),
    ];
    if matches!(
        plan.definition_owner(),
        scoop_lir::RegistrationDefinitionOwner::Odr { .. }
    ) {
        let key = DigestNodeKey::lir_definition(plan.registration_primary_atom());
        let lir = digest_plan
            .nodes()
            .iter()
            .find(|node| *node.key() == key)
            .ok_or(
                StrongImmortalObjectRegistrationValidationError::DigestPlanMismatch {
                    object: plan.object(),
                    kind: Failure::RegistrationDirectInputs,
                },
            )?;
        expected_inputs.push(DigestInputRefV1::from_node(lir));
    }
    expected_inputs.sort_unstable();
    if registration.direct_inputs() != expected_inputs {
        return digest_error(plan.object(), Failure::RegistrationDirectInputs);
    }
    if registration.patch_intents().len() != 1
        || registration.patch_intents()[0].id() != plan.registration_definition_patch()
    {
        return digest_error(plan.object(), Failure::RegistrationPatchSet);
    }
    Ok(())
}

fn digest_error<T>(
    object: PersistentImmortalObjectId,
    kind: ImmortalObjectRegistrationDigestPlanFailureV1,
) -> Result<T, StrongImmortalObjectRegistrationValidationError> {
    Err(StrongImmortalObjectRegistrationValidationError::DigestPlanMismatch { object, kind })
}
