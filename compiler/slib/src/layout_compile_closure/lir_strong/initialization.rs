//! Join emitted delegate units to their source roles at the artifact boundary.

use scoop_identity::{
    CallableBodyKey, CallableBodyKeyKind, DefinitionOwner, GeneratedCallableKey,
    InitializationCallableRole, InitializationUnitKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, ObjectDefinitionPlanOwner, OdrGroupId, OdrMemberDiscriminator,
    OdrMemberKey, PersistentCallableBodyId, PersistentInitializationUnitId, SpecializationKey,
    StaticStorageKey, StorageRole, ValidatedIdentityGraph,
};
use scoop_lir::{ConeProductionSectionV2, RegistrationDefinitionOwner};

use super::SharedLirStrongProductionError as Error;

pub(super) fn validate(
    production: &ConeProductionSectionV2,
    graph: &ValidatedIdentityGraph,
) -> Result<(), Error> {
    for plan in production.initialization_registrations().registrations() {
        let unit = plan.semantic().unit();
        let key = graph.canonical_key::<_, InitializationUnitKey>(unit)?;
        let Some(specialization) = key.specialization_key() else {
            if plan.definition_owner() != RegistrationDefinitionOwner::Strong {
                return Err(invalid(unit, "owner"));
            }
            continue;
        };
        let RegistrationDefinitionOwner::Odr { group, .. } = plan.definition_owner() else {
            return Err(invalid(unit, "owner"));
        };
        if *graph.canonical_key::<_, SpecializationKey>(group)? != specialization {
            return Err(invalid(unit, "application"));
        }
        if !matches!(
            plan.schedule(),
            scoop_lir::StrongInitializationRegistrationSchedulePlanV1::LazyAccess
        ) {
            return Err(invalid(unit, "schedule"));
        }

        let value_key = graph.canonical_key::<_, StaticStorageKey>(plan.storage().storage())?;
        if value_key.owner() != DefinitionOwner::InitializationUnit(unit)
            || !matches!(
                value_key.role(),
                StorageRole::PropertyDelegate | StorageRole::StaticPlaceToken
            )
        {
            return Err(invalid(unit, "storage"));
        }
        for (field, definition) in [
            ("cell", plan.cell_definition_plan()),
            ("registration", plan.registration_definition_plan()),
            ("initializer", plan.initializer().body_definition_plan()),
            (
                "initializer_registration",
                plan.initializer().registration_definition_plan(),
            ),
            ("ensure", plan.ensure().body_definition_plan()),
            (
                "ensure_registration",
                plan.ensure().registration_definition_plan(),
            ),
        ] {
            definition_group(graph, unit, group, definition, field)?;
        }
        for (field, storage) in [
            ("storage", plan.storage()),
            ("failure_root", plan.failure_root()),
        ] {
            let storage = production
                .static_storage_registrations()
                .registrations()
                .iter()
                .find(|candidate| candidate.semantic().storage() == storage.storage())
                .ok_or_else(|| invalid(unit, field))?;
            definition_group(graph, unit, group, storage.storage_definition_plan(), field)?;
            definition_group(
                graph,
                unit,
                group,
                storage.registration_definition_plan(),
                field,
            )?;
        }
        if !callable_role(
            graph,
            &key,
            plan.initializer().body(),
            InitializationCallableRole::Initializer,
        )? {
            return Err(invalid(unit, "initializer_role"));
        }
        if !callable_role(
            graph,
            &key,
            plan.ensure().body(),
            InitializationCallableRole::Ensure,
        )? {
            return Err(invalid(unit, "ensure_role"));
        }
    }
    Ok(())
}

fn definition_group(
    graph: &ValidatedIdentityGraph,
    unit: PersistentInitializationUnitId,
    group: OdrGroupId,
    definition: ObjectDefinitionPlanId,
    field: &'static str,
) -> Result<(), Error> {
    let key = graph.canonical_key::<_, ObjectDefinitionPlanKey>(definition)?;
    let ObjectDefinitionPlanOwner::Odr { member } = key.owner() else {
        return Err(invalid(unit, field));
    };
    if graph.canonical_key::<_, OdrMemberKey>(member)?.group() != group {
        return Err(invalid(unit, field));
    }
    Ok(())
}

fn callable_role(
    graph: &ValidatedIdentityGraph,
    unit: &InitializationUnitKey,
    body: PersistentCallableBodyId,
    role: InitializationCallableRole,
) -> Result<bool, Error> {
    let body = graph.canonical_key::<_, CallableBodyKey>(body)?;
    let CallableBodyKeyKind::Odr(member) = body.kind() else {
        return Ok(false);
    };
    let member = graph.canonical_key::<_, OdrMemberKey>(member.member())?;
    let OdrMemberDiscriminator::GeneratedCallable(callable) = member.discriminator() else {
        return Ok(false);
    };
    let callable = graph.canonical_key::<_, GeneratedCallableKey>(*callable)?;
    let GeneratedCallableKey::Initialization {
        unit: source,
        role: actual,
    } = *callable
    else {
        return Ok(false);
    };
    let source = graph.canonical_key::<_, InitializationUnitKey>(source)?;
    Ok(actual == role
        && matches!((unit, source.as_ref()),
        (InitializationUnitKey::GenericDelegatedExtensionApplication { property, .. }, InitializationUnitKey::ExtensionProperty(source)) if property == source))
}

fn invalid(unit: PersistentInitializationUnitId, field: &'static str) -> Error {
    Error::InitializationRelation { unit, field }
}
