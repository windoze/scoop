use super::*;
use scoop_wire::WirePath;

pub(super) fn validate(
    production: &ReplayedStrongProductionSectionV2,
    section: &crate::CrossConeLayoutAbiSectionV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), StrongProductionLayoutJoinError> {
    meter.charge_work(
        (section.callables().records().len() as u64).saturating_mul(
            production
                .callable_registrations()
                .registrations()
                .len()
                .max(1) as u64,
        ),
        &WirePath::root(),
    )?;
    for record in section.callables().records() {
        let target = record.target();
        let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(target))?;
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
