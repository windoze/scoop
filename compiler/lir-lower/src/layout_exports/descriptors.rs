use super::*;

pub(super) fn lower(
    input: LayoutAbiExportInputV1<'_>,
    layouts: &lir::CanonicalExactLayoutExportsV1,
    diagnostics: &impl scoop_identity::ExactTypeDiagnosticGraph,
    meter: &mut BudgetMeter,
) -> Result<lir::CanonicalExactDescriptorExportsV1, Error> {
    let registrations = input
        .registration
        .registration_production()
        .types()
        .registrations();
    let actual = &input.lir.module().meta.type_descriptors;
    if registrations.len() != actual.len() {
        return Err(Error::DescriptorSet);
    }
    let mut records = reserve(registrations.len(), meter)?;
    for registration in registrations {
        let exact = registration.exact_type();
        meter.charge_work(actual.len() as u64, &WirePath::root())?;
        let physical = actual
            .iter()
            .find(|(_, descriptor)| descriptor.identity.exact_type() == exact)
            .map(|(_, descriptor)| descriptor)
            .ok_or(Error::DescriptorProduction(exact))?;
        let semantic = registration.semantic();
        if physical.instance_layout.layout_record().id() != semantic.instance_layout()
            || &physical.instance_shape != semantic.instance_shape()
            || physical.inline_scan != semantic.inline_scan()
            || physical.diagnostic_name != semantic.diagnostic_name()
        {
            return Err(Error::DescriptorProduction(exact));
        }
        records.push(lir::ExactDescriptorExportV1::replay(
            input.lir.module().meta.target_profile,
            layouts,
            registration,
            diagnostics,
            input.lir.foundation(),
            meter,
        )?);
    }
    Ok(lir::CanonicalExactDescriptorExportsV1::try_new(
        input.lir.module().meta.target_profile,
        input.lir.foundation(),
        records,
        meter,
    )?)
}
