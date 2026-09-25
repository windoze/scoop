use super::*;

pub(super) fn validate(
    production: &ReplayedStrongProductionSectionV2,
    section: &crate::LayoutAbiExportConstituentsV1,
) -> Result<(), StrongProductionLayoutJoinError> {
    let registrations = production.type_registrations().registrations();

    for record in section.shape_support().records() {
        let roles = record.roles();
        let source_descriptor = roles.type_descriptor().available();
        let source_registration = roles.type_registration().available();
        if source_descriptor.is_none_or(|value| value.semantic_id() != record.exact())
            || source_registration.is_none_or(|value| value.semantic_id() != record.exact())
            || !matches_registration(
                record.exact(),
                source_descriptor,
                source_registration,
                registrations,
            )
        {
            return Err(StrongProductionLayoutJoinError::ShapeSupportProduction(
                record.source_nominal(),
            ));
        }
        for helper in [
            roles.boxed_value().available(),
            roles.coroutine_step().available(),
            roles.coroutine_slot().available(),
        ]
        .into_iter()
        .flatten()
        {
            let descriptor = helper.descriptor();
            let registration = helper.registration();
            if !matches_registration(
                helper.exact(),
                Some(&descriptor),
                Some(&registration),
                registrations,
            ) {
                return Err(StrongProductionLayoutJoinError::ShapeSupportProduction(
                    record.source_nominal(),
                ));
            }
        }
    }
    Ok(())
}

fn matches_registration(
    exact: PersistentExactTypeId,
    descriptor: Option<&crate::StrongShapeDefinitionV1<PersistentExactTypeId>>,
    exported: Option<&crate::StrongShapeRegistrationV1<PersistentExactTypeId>>,
    registrations: &[crate::StrongTypeRegistrationPlanV2],
) -> bool {
    let Some((descriptor, exported, registration)) =
        descriptor.zip(exported).and_then(|(descriptor, exported)| {
            registrations
                .iter()
                .find(|registration| registration.exact_type() == exact)
                .map(|registration| (descriptor, exported, registration))
        })
    else {
        return false;
    };
    descriptor.semantic_id() == exact
        && descriptor.definition_plan() == registration.descriptor_definition_plan()
        && descriptor.symbol() == registration.descriptor_symbol()
        && exported.semantic_id() == exact
        && exported.definition_plan() == registration.definition_plan()
        && exported.symbol() == registration.symbol()
        && exported.fingerprint_node() == registration.registration_fingerprint_node()
}
