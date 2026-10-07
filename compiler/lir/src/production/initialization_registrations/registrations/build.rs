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
    let registration_symbol = require_symbol(
        foundation,
        PersistentSymbolKey::InitializationRegistration(unit),
        identity.owner().linkage(),
    )?;

    let cell_definition =
        require_definition(foundation, entity, StrongDefinitionRole::InitializationCell)?;
    let cell_primary_atom = require_primary_atom(foundation, cell_definition.id())?;
    require_associated_atoms(foundation, unit, cell_definition.id(), [])?;
    let cell_symbol = require_symbol(
        foundation,
        PersistentSymbolKey::InitializationCell(unit),
        identity.owner().linkage(),
    )?;

    let [diagnostic_atom] = require_associated_atoms(
        foundation,
        unit,
        registration_definition.id(),
        [ObjectDefinitionAtomKey::new(
            registration_definition.id(),
            DefinitionAtomRole::AddressTakenConstant,
            DefinitionAtomSubkey::InitializationUnit(unit),
        )],
    )?;

    let storage = require_static_storage(foundation, identities, semantic.storage())?;
    let failure_root = require_static_storage(foundation, identities, semantic.failure_root())?;
    let initializer = require_callable(foundation, identities, semantic.initializer(), digests)?;
    let ensure = require_callable(foundation, identities, semantic.ensure(), digests)?;

    let schedule = match semantic.schedule() {
        StrongInitializationSchedulePlanV1::EagerStartup { gateway } => {
            let gateway = require_callable(foundation, identities, gateway, digests)?;
            let gateway_node = require_digest_node(
                digests,
                DigestNodeKey::object_definition(gateway.body_primary_atom()),
            )?;
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
    Ok(StrongInitializationUnitRegistrationPlan {
        definition_owner: identity.owner(),
        semantic: semantic.clone(),
        registration_symbol,
        registration_definition_plan: registration_definition.id(),
        registration_primary_atom,
        cell_symbol,
        cell_definition_plan: cell_definition.id(),
        cell_primary_atom,
        diagnostic_atom,
        storage,
        failure_root,
        initializer,
        ensure,
        schedule,
    })
}
