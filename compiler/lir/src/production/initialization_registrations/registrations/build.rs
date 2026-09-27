//! Join initialization semantics to physical definitions and digest plans.

use super::validation::*;
use super::*;

pub(super) fn build_registration<D: crate::StrongInitializationDependencyReference>(
    foundation: &ConeLirFoundation,
    identities: &RegistrationIdentitySurfaceV1,
    semantic: &StrongInitializationUnitSemanticPlan<D>,
    identity: &crate::RegistrationIdentityV1<PersistentInitializationUnitId>,
    digests: &DigestFinalizationPlanV1,
) -> Result<
    StrongInitializationUnitRegistrationPlan<D>,
    StrongInitializationUnitRegistrationPlanBuildError,
> {
    let unit = semantic.unit();
    for (index, dependency) in semantic.dependencies().iter().enumerate() {
        let id = dependency.unit_id();
        if id == unit {
            return Err(StrongInitializationUnitRegistrationPlanBuildError::SelfDependency(unit));
        }
        if index > 0 && semantic.dependencies()[index - 1].unit_id() >= id {
            return Err(
                StrongInitializationUnitRegistrationPlanBuildError::DependencyOrder { unit, index },
            );
        }
        if !dependency.has_valid_provider_role(foundation.producer()) {
            return Err(
                StrongInitializationUnitRegistrationPlanBuildError::DependencyProviderRole {
                    unit,
                    dependency: id,
                },
            );
        }
    }
    let entity = StrongDefinitionEntity::initialization_unit(unit);
    let registration_definition = require_definition(
        foundation,
        entity,
        StrongDefinitionRole::InitializationRegistration,
    )?;
    if registration_definition.id() != identity.definition_plan() {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::RegistrationDefinitionMismatch {
                unit,
                expected: registration_definition.id(),
                actual: identity.definition_plan(),
            },
        );
    }
    let registration_primary_atom = require_primary_atom(foundation, registration_definition.id())?;
    require_associated_atoms(foundation, unit, registration_definition.id(), [])?;
    let registration_symbol = require_symbol(
        foundation,
        PersistentSymbolKey::InitializationRegistration(unit),
    )?;

    let cell_definition =
        require_definition(foundation, entity, StrongDefinitionRole::InitializationCell)?;
    let cell_primary_atom = require_primary_atom(foundation, cell_definition.id())?;
    require_associated_atoms(foundation, unit, cell_definition.id(), [])?;
    let cell_symbol = require_symbol(foundation, PersistentSymbolKey::InitializationCell(unit))?;

    let descriptor_definition = require_definition(
        foundation,
        entity,
        StrongDefinitionRole::InitializationDescriptor,
    )?;
    let descriptor_primary_atom = require_primary_atom(foundation, descriptor_definition.id())?;
    let [diagnostic_atom] = require_associated_atoms(
        foundation,
        unit,
        descriptor_definition.id(),
        [ObjectDefinitionAtomKey::new(
            descriptor_definition.id(),
            DefinitionAtomRole::AddressTakenConstant,
            DefinitionAtomSubkey::InitializationUnit(unit),
        )],
    )?;
    let descriptor_symbol = require_symbol(
        foundation,
        PersistentSymbolKey::InitializationDescriptor(unit),
    )?;

    let storage = require_static_storage(foundation, identities, semantic.storage(), digests)?;
    let failure_root =
        require_static_storage(foundation, identities, semantic.failure_root(), digests)?;
    let initializer = require_callable(foundation, identities, semantic.initializer(), digests)?;
    let ensure = require_callable(foundation, identities, semantic.ensure(), digests)?;

    let registration_object = require_leaf_object_node(
        digests,
        registration_primary_atom,
        InitializationObjectLeafV1::Registration,
    )?;
    let cell_object =
        require_leaf_object_node(digests, cell_primary_atom, InitializationObjectLeafV1::Cell)?;
    let descriptor_object = require_leaf_object_node(
        digests,
        descriptor_primary_atom,
        InitializationObjectLeafV1::Descriptor,
    )?;
    let registration_fingerprint = require_digest_node(
        digests,
        DigestNodeKey::strong_registration(registration_definition.id()),
    )?;
    if registration_fingerprint.id() != identity.fingerprint_node() {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::RegistrationDigestMismatch {
                unit,
                expected: registration_fingerprint.id(),
                actual: identity.fingerprint_node(),
            },
        );
    }

    let mut expected_inputs = vec![
        DigestInputRefV1::from_node(registration_object),
        DigestInputRefV1::from_node(cell_object),
        DigestInputRefV1::from_node(descriptor_object),
    ];
    let schedule = match semantic.schedule() {
        StrongInitializationSchedulePlanV1::EagerStartup { gateway } => {
            let gateway = require_callable(foundation, identities, gateway, digests)?;
            let gateway_node = require_digest_node(
                digests,
                DigestNodeKey::object_definition(gateway.body_primary_atom()),
            )?;
            expected_inputs.push(DigestInputRefV1::from_node(gateway_node));
            let expected_patch = DigestPatchIntentKey::new(
                gateway_node.id(),
                registration_definition.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::GatewayDefinition,
            );
            let gateway_definition_patch =
                require_gateway_patch(digests, registration_definition.id(), expected_patch)?;
            StrongInitializationRegistrationSchedulePlanV1::EagerStartup {
                gateway: Box::new(gateway),
                gateway_definition_patch,
            }
        }
        StrongInitializationSchedulePlanV1::LazyAccess => {
            require_no_gateway_patches(digests, registration_definition.id())?;
            StrongInitializationRegistrationSchedulePlanV1::LazyAccess
        }
    };
    expected_inputs.sort_unstable();
    if registration_fingerprint.direct_inputs() != expected_inputs {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::DirectInputs {
                node: registration_fingerprint.id(),
                expected: expected_inputs,
                actual: registration_fingerprint.direct_inputs().to_vec(),
            },
        );
    }
    let registration_definition_patch = require_only_patch(
        registration_fingerprint,
        DigestPatchIntentKey::new(
            registration_fingerprint.id(),
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        ),
    )?;

    Ok(StrongInitializationUnitRegistrationPlan {
        semantic: semantic.clone(),
        registration_symbol,
        registration_definition_plan: registration_definition.id(),
        registration_primary_atom,
        cell_symbol,
        cell_definition_plan: cell_definition.id(),
        cell_primary_atom,
        descriptor_symbol,
        descriptor_definition_plan: descriptor_definition.id(),
        descriptor_primary_atom,
        diagnostic_atom,
        storage,
        failure_root,
        initializer,
        ensure,
        schedule,
        registration_object_node: registration_object.id(),
        cell_definition_node: cell_object.id(),
        descriptor_definition_node: descriptor_object.id(),
        registration_fingerprint_node: registration_fingerprint.id(),
        registration_definition_patch,
    })
}
