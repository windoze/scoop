use super::*;
use scoop_wire::WirePath;

pub(super) fn validate(
    production: &ReplayedStrongProductionSectionV2,
    section: &crate::CrossConeLayoutAbiSectionV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), StrongProductionLayoutJoinError> {
    validate_descriptors(production, section, meter)?;
    validate_dispatch(production, section, meter)
}

fn validate_descriptors(
    production: &ReplayedStrongProductionSectionV2,
    section: &crate::CrossConeLayoutAbiSectionV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), StrongProductionLayoutJoinError> {
    let path = WirePath::root();
    meter.charge_work(
        (section.descriptors().records().len() as u64)
            .saturating_mul(production.type_registrations().registrations().len().max(1) as u64)
            .saturating_add(
                (section.descriptors().records().len() as u64)
                    .saturating_mul(section.layouts().records().len().max(1) as u64),
            ),
        &path,
    )?;
    for record in section.descriptors().records() {
        let exact = record.exact();
        let registration = production
            .type_registrations()
            .registrations()
            .iter()
            .find(|registration| registration.exact_type() == exact)
            .ok_or(StrongProductionLayoutJoinError::MissingDescriptorProduction(exact))?;
        let semantic = registration.semantic();
        let interfaces_match = record.ancestry().interfaces().len() == semantic.itables().len()
            && record
                .ancestry()
                .interfaces()
                .iter()
                .zip(semantic.itables())
                .all(|(actual, expected)| *actual == expected.interface());
        let tables_match = record.dispatch().itables().len() == semantic.itables().len()
            && record
                .dispatch()
                .itables()
                .iter()
                .zip(semantic.itables())
                .all(|(actual, expected)| {
                    actual.interface() == expected.interface() && actual.table() == expected.table()
                });
        let physical = record.physical_definition();
        let layout_physical = record.instance_layout().identity().physical_definition();
        let definition = record.definition();
        let exported_registration = record.registration();
        let instance_scan = section
            .layouts()
            .get(semantic.instance_layout())
            .map(crate::ExactLayoutExportV1::scan);
        if record.value_layout().identity().exact() != exact
            || record.instance_layout().identity().exact() != exact
            || record.instance_layout().identity().layout() != semantic.instance_layout()
            || instance_scan != Some(semantic.instance_scan())
            || record.shape() != semantic.instance_shape()
            || record.object_scan() != semantic.instance_shape().object_scan()
            || record.ancestry().parent() != semantic.parent()
            || !interfaces_match
            || record.dispatch().vtable() != semantic.vtable().table()
            || !tables_match
            || record.diagnostic_name().as_str() != semantic.diagnostic_name()
            || registration.descriptor_definition_plan() != physical.definition()
            || registration.descriptor_primary_atom() != physical.primary()
            || registration.descriptor_symbol() != physical.symbol()
            || registration.layout() != record.instance_layout().identity().layout()
            || registration.layout_definition_plan() != layout_physical.definition()
            || registration.layout_primary_atom() != layout_physical.primary()
            || registration.layout_symbol() != layout_physical.symbol()
            || !matches_inline_scan(registration, section.layouts())
            || definition.semantic_id() != exact
            || definition.definition_plan() != physical.definition()
            || definition.symbol() != physical.symbol()
            || exported_registration.semantic_id() != exact
            || exported_registration.definition_plan() != registration.definition_plan()
            || exported_registration.symbol() != registration.symbol()
            || exported_registration.fingerprint_node()
                != registration.registration_fingerprint_node()
        {
            return Err(StrongProductionLayoutJoinError::DescriptorProduction(exact));
        }
    }
    Ok(())
}

fn validate_dispatch(
    production: &ReplayedStrongProductionSectionV2,
    section: &crate::CrossConeLayoutAbiSectionV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), StrongProductionLayoutJoinError> {
    meter.charge_work(
        (section.dispatch().records().len() as u64)
            .saturating_mul(production.type_registrations().registrations().len().max(1) as u64),
        &WirePath::root(),
    )?;
    for record in section.dispatch().records() {
        let registration = production
            .type_registrations()
            .registrations()
            .iter()
            .find(|registration| registration.exact_type() == record.owner_exact())
            .ok_or(StrongProductionLayoutJoinError::MissingDispatchProduction(
                record.table(),
            ))?;
        let semantic = registration.semantic();
        let slots = match record.role() {
            crate::ExactDispatchRoleV1::Vtable if semantic.vtable().table() == record.table() => {
                Some(semantic.vtable().slots())
            }
            crate::ExactDispatchRoleV1::Itable { interface_exact } => semantic
                .itables()
                .iter()
                .find(|table| {
                    table.table() == record.table()
                        && table.interface().exact_type() == interface_exact
                })
                .map(|table| table.slots()),
            _ => None,
        }
        .ok_or(StrongProductionLayoutJoinError::DispatchProduction(
            record.table(),
        ))?;
        if !layouts::matches_definition(
            production.canonical_definitions(),
            record.physical_definition(),
        ) || record.entries().len() != slots.len()
            || !record
                .entries()
                .iter()
                .zip(slots)
                .all(|(entry, slot)| entry.abi() == *slot)
        {
            return Err(StrongProductionLayoutJoinError::DispatchProduction(
                record.table(),
            ));
        }
    }
    Ok(())
}

fn matches_inline_scan(
    registration: &crate::StrongTypeRegistrationPlanV2,
    layouts: &crate::CanonicalExactLayoutExportsV1,
) -> bool {
    match registration.inline_scan() {
        crate::StrongTypeDescriptorInlineScanPlanV1::Null => {
            matches!(
                registration.semantic().inline_scan(),
                crate::TypeDescriptorInlineScanV1::Null
            )
        }
        crate::StrongTypeDescriptorInlineScanPlanV1::Defined {
            scan,
            definition_plan,
        } => {
            matches!(
                registration.semantic().inline_scan(),
                crate::TypeDescriptorInlineScanV1::Defined(actual) if actual == scan
            ) && layouts.records().iter().any(|record| {
                record.scan() == scan && record.scan_definition().definition() == definition_plan
            })
        }
    }
}
