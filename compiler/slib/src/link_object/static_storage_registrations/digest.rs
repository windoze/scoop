use scoop_identity::{DigestNodeKey, DigestPatchIntentKey, DigestSemanticFieldRole};
use scoop_lir::{
    DigestInputRefV1, DigestNodeV1, StrongDigestFinalizationPlanV1,
    StrongStaticStorageRegistrationPlanV1,
};

use super::{
    StaticStorageRegistrationDigestPlanFailureV1, StrongStaticStorageRegistrationValidationError,
};

pub(super) fn validate_digest_graph(
    digest_plan: &StrongDigestFinalizationPlanV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    use StaticStorageRegistrationDigestPlanFailureV1 as Failure;

    let registration_object = require_node(
        digest_plan,
        DigestNodeKey::object_definition(plan.registration_primary_atom()),
        plan,
        Failure::MissingRegistrationObjectDefinitionNode,
    )?;
    validate_leaf(
        registration_object,
        plan.registration_object_node(),
        plan,
        Failure::RegistrationObjectDefinitionNodeIdentity,
        Failure::RegistrationObjectDefinitionDirectInputs,
        Failure::RegistrationObjectDefinitionPatchSet,
    )?;

    let storage_object = require_node(
        digest_plan,
        DigestNodeKey::object_definition(plan.storage_primary_atom()),
        plan,
        Failure::MissingStorageObjectDefinitionNode,
    )?;
    validate_leaf(
        storage_object,
        plan.storage_definition_node(),
        plan,
        Failure::StorageObjectDefinitionNodeIdentity,
        Failure::StorageObjectDefinitionDirectInputs,
        Failure::StorageObjectDefinitionPatchSet,
    )?;

    let layout = require_node(
        digest_plan,
        DigestNodeKey::layout(plan.semantic().layout()),
        plan,
        Failure::MissingLayoutNode,
    )?;
    if layout.id() != plan.layout_fingerprint_node() {
        return digest_error(plan, Failure::LayoutNodeIdentity);
    }
    if !layout.direct_inputs().is_empty() {
        return digest_error(plan, Failure::LayoutDirectInputs);
    }
    validate_shape_patch(
        layout,
        plan,
        plan.layout_fingerprint_patch(),
        DigestSemanticFieldRole::Layout,
        Failure::LayoutPatch,
    )?;

    let scan = require_node(
        digest_plan,
        DigestNodeKey::scan(plan.semantic().scan()),
        plan,
        Failure::MissingScanNode,
    )?;
    if scan.id() != plan.scan_fingerprint_node() {
        return digest_error(plan, Failure::ScanNodeIdentity);
    }
    if !scan.direct_inputs().is_empty() {
        return digest_error(plan, Failure::ScanDirectInputs);
    }
    validate_shape_patch(
        scan,
        plan,
        plan.scan_fingerprint_patch(),
        DigestSemanticFieldRole::Scan,
        Failure::ScanPatch,
    )?;

    let registration = require_node(
        digest_plan,
        DigestNodeKey::strong_registration(plan.registration_definition_plan()),
        plan,
        Failure::MissingRegistrationNode,
    )?;
    if registration.id() != plan.registration_fingerprint_node() {
        return digest_error(plan, Failure::RegistrationNodeIdentity);
    }
    let mut expected_inputs = vec![
        DigestInputRefV1::from_node(registration_object),
        DigestInputRefV1::from_node(storage_object),
        DigestInputRefV1::from_node(layout),
        DigestInputRefV1::from_node(scan),
    ];
    expected_inputs.sort_unstable();
    if registration.direct_inputs() != expected_inputs {
        return digest_error(plan, Failure::RegistrationDirectInputs);
    }
    let expected_patch = DigestPatchIntentKey::new(
        registration.id(),
        plan.registration_definition_plan(),
        scoop_identity::DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::RegistrationDefinition,
    );
    if registration.patch_intents().len() != 1
        || registration.patch_intents()[0].id() != plan.registration_definition_patch()
        || registration.patch_intents()[0].key() != &expected_patch
    {
        return digest_error(plan, Failure::RegistrationPatchSet);
    }
    Ok(())
}

fn require_node<'a>(
    digest_plan: &'a StrongDigestFinalizationPlanV1,
    key: DigestNodeKey,
    plan: &StrongStaticStorageRegistrationPlanV1,
    failure: StaticStorageRegistrationDigestPlanFailureV1,
) -> Result<&'a DigestNodeV1, StrongStaticStorageRegistrationValidationError> {
    digest_plan
        .nodes()
        .iter()
        .find(|node| node.key() == &key)
        .ok_or(
            StrongStaticStorageRegistrationValidationError::DigestPlanMismatch {
                storage: plan.semantic().storage(),
                kind: failure,
            },
        )
}

fn validate_leaf(
    node: &DigestNodeV1,
    expected_id: scoop_identity::DigestNodeId,
    plan: &StrongStaticStorageRegistrationPlanV1,
    identity_failure: StaticStorageRegistrationDigestPlanFailureV1,
    input_failure: StaticStorageRegistrationDigestPlanFailureV1,
    patch_failure: StaticStorageRegistrationDigestPlanFailureV1,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    if node.id() != expected_id {
        return digest_error(plan, identity_failure);
    }
    if !node.direct_inputs().is_empty() {
        return digest_error(plan, input_failure);
    }
    if !node.patch_intents().is_empty() {
        return digest_error(plan, patch_failure);
    }
    Ok(())
}

fn validate_shape_patch(
    node: &DigestNodeV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
    expected_id: scoop_identity::DigestPatchIntentId,
    field: DigestSemanticFieldRole,
    failure: StaticStorageRegistrationDigestPlanFailureV1,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    let expected_key = DigestPatchIntentKey::new(
        node.id(),
        plan.registration_definition_plan(),
        scoop_identity::DefinitionAtomRole::Primary,
        field,
    );
    if !node
        .patch_intents()
        .iter()
        .any(|patch| patch.id() == expected_id && patch.key() == &expected_key)
    {
        return digest_error(plan, failure);
    }
    Ok(())
}

fn digest_error<T>(
    plan: &StrongStaticStorageRegistrationPlanV1,
    kind: StaticStorageRegistrationDigestPlanFailureV1,
) -> Result<T, StrongStaticStorageRegistrationValidationError> {
    Err(
        StrongStaticStorageRegistrationValidationError::DigestPlanMismatch {
            storage: plan.semantic().storage(),
            kind,
        },
    )
}
