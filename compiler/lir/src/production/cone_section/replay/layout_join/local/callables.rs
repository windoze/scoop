use super::*;

pub(super) fn validate(
    production: &ConeProductionSectionV2,
    section: &crate::LayoutAbiExportConstituentsV1,
) -> Result<(), StrongProductionLayoutJoinError> {
    for record in section.callables().records() {
        let target = record.target();
        let body = PersistentCallableBodyId::from_key(&target.body_key())?;
        let registration = production
            .callable_registrations()
            .registrations()
            .iter()
            .find(|registration| registration.body() == body)
            .ok_or(StrongProductionLayoutJoinError::MissingCallableProduction(
                target,
            ))?;
        let physical = record.physical_definition();
        if record.definition().semantic_id() != body
            || registration.body_definition_plan() != physical.definition()
            || registration.body_primary_atom() != physical.primary()
            || registration.entry_symbol() != physical.symbol()
        {
            return Err(StrongProductionLayoutJoinError::CallableProduction(target));
        }
    }
    Ok(())
}
