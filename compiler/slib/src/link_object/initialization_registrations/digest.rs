use scoop_identity::{
    DefinitionAtomRole, DigestNodeKey, DigestPatchIntentKey, DigestSemanticFieldRole,
};
use scoop_lir::{
    DigestInputRefV1, StrongDigestFinalizationPlanV1,
    StrongInitializationRegistrationSchedulePlanV1, StrongInitializationUnitRegistrationPlan,
};

use super::{
    InitializationRegistrationDigestPlanFailureV1, StrongInitializationRegistrationValidationError,
};

pub(super) fn validate_digest_graph<D>(
    digest_plan: &StrongDigestFinalizationPlanV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> Result<(), StrongInitializationRegistrationValidationError> {
    use InitializationRegistrationDigestPlanFailureV1 as Failure;

    let registration_object = require_leaf(
        digest_plan,
        plan,
        DigestNodeKey::object_definition(plan.registration_primary_atom()),
        plan.registration_object_node(),
        Failure::MissingRegistrationObjectDefinitionNode,
        Failure::RegistrationObjectDefinitionNodeIdentity,
        Failure::RegistrationObjectDefinitionDirectInputs,
        Failure::RegistrationObjectDefinitionPatchSet,
    )?;
    let cell_object = require_leaf(
        digest_plan,
        plan,
        DigestNodeKey::object_definition(plan.cell_primary_atom()),
        plan.cell_definition_node(),
        Failure::MissingCellObjectDefinitionNode,
        Failure::CellObjectDefinitionNodeIdentity,
        Failure::CellObjectDefinitionDirectInputs,
        Failure::CellObjectDefinitionPatchSet,
    )?;
    let descriptor_object = require_leaf(
        digest_plan,
        plan,
        DigestNodeKey::object_definition(plan.descriptor_primary_atom()),
        plan.descriptor_definition_node(),
        Failure::MissingDescriptorObjectDefinitionNode,
        Failure::DescriptorObjectDefinitionNodeIdentity,
        Failure::DescriptorObjectDefinitionDirectInputs,
        Failure::DescriptorObjectDefinitionPatchSet,
    )?;

    let registration = digest_plan
        .nodes()
        .iter()
        .find(|node| {
            node.key() == &DigestNodeKey::strong_registration(plan.registration_definition_plan())
        })
        .ok_or_else(|| mismatch(plan, Failure::MissingRegistrationNode))?;
    if registration.id() != plan.registration_fingerprint_node() {
        return Err(mismatch(plan, Failure::RegistrationNodeIdentity));
    }

    let mut expected_inputs = vec![
        DigestInputRefV1::from_node(registration_object),
        DigestInputRefV1::from_node(cell_object),
        DigestInputRefV1::from_node(descriptor_object),
    ];
    let expected_gateway_patch = match plan.schedule() {
        StrongInitializationRegistrationSchedulePlanV1::EagerStartup {
            gateway,
            gateway_definition_patch,
        } => {
            let gateway_node = digest_plan
                .nodes()
                .iter()
                .find(|node| {
                    node.key() == &DigestNodeKey::object_definition(gateway.body_primary_atom())
                })
                .ok_or_else(|| mismatch(plan, Failure::MissingGatewayObjectDefinitionNode))?;
            if gateway_node.id() != gateway.body_definition_node() {
                return Err(mismatch(plan, Failure::GatewayObjectDefinitionNodeIdentity));
            }
            expected_inputs.push(DigestInputRefV1::from_node(gateway_node));
            Some((gateway_node.id(), *gateway_definition_patch))
        }
        StrongInitializationRegistrationSchedulePlanV1::LazyAccess => None,
    };
    expected_inputs.sort_unstable();
    if registration.direct_inputs() != expected_inputs {
        return Err(mismatch(plan, Failure::RegistrationDirectInputs));
    }
    if registration.patch_intents().len() != 1
        || registration.patch_intents()[0].id() != plan.registration_definition_patch()
    {
        return Err(mismatch(plan, Failure::RegistrationPatchSet));
    }

    let actual_gateway_patches = digest_plan
        .nodes()
        .iter()
        .flat_map(|node| node.patch_intents())
        .filter(|patch| {
            patch.key().target_definition() == plan.registration_definition_plan()
                && patch.key().atom_role() == DefinitionAtomRole::Primary
                && patch.key().semantic_field_role() == DigestSemanticFieldRole::GatewayDefinition
        })
        .collect::<Vec<_>>();
    let gateway_patch_is_exact = match (expected_gateway_patch, actual_gateway_patches.as_slice()) {
        (None, []) => true,
        (Some((source, expected_id)), [actual]) => {
            actual.id() == expected_id
                && actual.key()
                    == &DigestPatchIntentKey::new(
                        source,
                        plan.registration_definition_plan(),
                        DefinitionAtomRole::Primary,
                        DigestSemanticFieldRole::GatewayDefinition,
                    )
        }
        _ => false,
    };
    if !gateway_patch_is_exact {
        return Err(mismatch(plan, Failure::GatewayDefinitionPatchSet));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn require_leaf<'a, D>(
    digest_plan: &'a StrongDigestFinalizationPlanV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    key: DigestNodeKey,
    expected_id: scoop_identity::DigestNodeId,
    missing: InitializationRegistrationDigestPlanFailureV1,
    identity: InitializationRegistrationDigestPlanFailureV1,
    direct_inputs: InitializationRegistrationDigestPlanFailureV1,
    patch_set: InitializationRegistrationDigestPlanFailureV1,
) -> Result<&'a scoop_lir::DigestNodeV1, StrongInitializationRegistrationValidationError> {
    let node = digest_plan
        .nodes()
        .iter()
        .find(|node| node.key() == &key)
        .ok_or_else(|| mismatch(plan, missing))?;
    if node.id() != expected_id {
        return Err(mismatch(plan, identity));
    }
    if !node.direct_inputs().is_empty() {
        return Err(mismatch(plan, direct_inputs));
    }
    if !node.patch_intents().is_empty() {
        return Err(mismatch(plan, patch_set));
    }
    Ok(node)
}

fn mismatch<D>(
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    kind: InitializationRegistrationDigestPlanFailureV1,
) -> StrongInitializationRegistrationValidationError {
    StrongInitializationRegistrationValidationError::DigestPlanMismatch {
        unit: plan.semantic().unit(),
        kind,
    }
}
