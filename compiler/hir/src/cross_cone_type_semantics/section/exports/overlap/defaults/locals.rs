use super::*;

pub(super) fn charge(
    locals: &CanonicalTemplateLocalTableV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<u64, WireError> {
    sequence(locals.records().len(), meter, path)?;
    let mut maximum = 16;
    for (index, local) in locals.records().iter().enumerate() {
        let at = path.clone().index(index as u64);
        maximum = maximum.max(selector(local.selector(), meter, &at)?);
        signature(local.value_type(), local.value_type(), meter, &at)?;
        if let TemplateLocalDefinitionV1::Source(value) = local.definition() {
            origin(value, meter, &at)?;
        }
    }
    Ok(maximum)
}
