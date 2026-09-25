use super::*;

mod callables;
mod layouts;
mod shape_support;
mod types;

pub(super) fn validate(
    production: &ReplayedStrongProductionSectionV2,
    section: &crate::LayoutAbiExportConstituentsV1,
    meter: &mut BudgetMeter,
) -> Result<(), StrongProductionLayoutJoinError> {
    meter.charge_work(9, &scoop_wire::WirePath::root())?;
    let provider = production.type_registrations().producer();
    for (component, actual) in [
        ("layout_abi", section.provider()),
        ("external_bridges", production.external_bridges().producer()),
        (
            "callable_registrations",
            production.callable_registrations().producer(),
        ),
        (
            "initialization_registrations",
            production.initialization_registrations().producer(),
        ),
        (
            "static_storage_registrations",
            production.static_storage_registrations().producer(),
        ),
        (
            "safepoint_registrations",
            production.safepoint_registrations().producer(),
        ),
        (
            "immortal_registrations",
            production.immortal_registrations().producer(),
        ),
        (
            "generated_bridge_plan",
            production.generated_bridge_plan().producer(),
        ),
    ] {
        if actual != provider {
            return Err(StrongProductionLayoutJoinError::Provider {
                expected: provider,
                actual,
                component,
            });
        }
    }
    if production.type_registrations().target() != &section.target_profile().wire_id() {
        return Err(StrongProductionLayoutJoinError::Target);
    }
    layouts::validate(production, section, meter)?;
    types::validate(production, section, meter)?;
    callables::validate(production, section, meter)?;
    shape_support::validate(production, section, meter)
}
