//! Match all six registration identity tables against actual object plans.

use super::*;

pub(super) fn registration_identities_match<D, C, I>(
    identities: &RegistrationIdentitySurfaceV1,
    registrations: &VerifiedStrongRegistrationPatchSetV1<D, C, I>,
) -> bool
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let static_storages = registrations
        .static_storages()
        .shapes()
        .registrations()
        .plan()
        .registrations();
    let immortal_objects = registrations
        .immortal_objects()
        .registrations()
        .plan()
        .registrations();
    let initializations = registrations
        .initializations()
        .registrations()
        .plan()
        .registrations();
    let types = registrations.types().registrations().plan().registrations();
    let safepoints = registrations
        .safepoints()
        .registrations()
        .plan()
        .registrations();
    let callables = registrations
        .callables()
        .body_objects()
        .registrations()
        .plan()
        .registrations();

    table_matches(
        identities.static_storages(),
        static_storages.iter().map(|plan| {
            (
                plan.semantic().storage(),
                plan.registration_definition_plan(),
            )
        }),
    ) && table_matches(
        identities.immortal_objects(),
        immortal_objects
            .iter()
            .map(|plan| (plan.object(), plan.registration_definition_plan())),
    ) && table_matches(
        identities.initialization_units(),
        initializations
            .iter()
            .map(|plan| (plan.semantic().unit(), plan.registration_definition_plan())),
    ) && table_matches(
        identities.type_registrations(),
        types
            .iter()
            .map(|plan| (plan.exact_type(), plan.definition_plan())),
    ) && table_matches(
        identities.safepoints(),
        safepoints
            .iter()
            .map(|plan| (plan.site(), plan.definition_plan())),
    ) && table_matches(
        identities.callables(),
        callables
            .iter()
            .map(|plan| (plan.body(), plan.definition_plan())),
    )
}

fn table_matches<I: scoop_identity::PersistentId>(
    identities: &[RegistrationIdentityV1<I>],
    plans: impl IntoIterator<Item = (I, ObjectDefinitionPlanId)>,
) -> bool {
    identities
        .iter()
        .map(|identity| (identity.semantic_id(), identity.definition_plan()))
        .eq(plans)
}
