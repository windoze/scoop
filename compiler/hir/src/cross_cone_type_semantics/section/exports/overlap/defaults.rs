use super::*;
use scoop_identity::{LocalValueSelector, StructuralDefinitionPath};

mod body;
mod locals;

pub(super) fn validate<E>(
    new: &CheckedProtectedDefaultTemplatesV1<'_>,
    public: &CrossConeHirInterfaceSectionV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    sequence(new.records().len(), meter, path)?;
    let old = public.default_templates().records();
    for (index, checked) in new.records().iter().enumerate() {
        let new = checked.template();
        let key = (new.key().owner(), new.key().parameter_position());
        lookup(old.len(), meter, path)?;
        if let Ok(old_index) = old.binary_search_by_key(&key, |record| {
            (record.key().owner(), record.key().parameter_position())
        }) {
            require(equal(
                new,
                &old[old_index],
                meter,
                &path.clone().index(index as u64),
            )?)?;
        }
    }
    Ok(())
}

pub(super) fn equal(
    new: &ProtectedDefaultTemplateV1,
    old: &ExportDefaultTemplateV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    meter.charge_work(128, path)?;
    if new.key().owner() != old.key().owner()
        || new.key().parameter_position() != old.key().parameter_position()
        || new.definition_root() != old.definition_root()
        || new.allows_suspend() != old.allows_suspend()
    {
        return Ok(false);
    }
    structural_path(new.definition_path(), meter, &path.clone().field(3))?;
    structural_path(old.definition_path(), meter, &path.clone().field(3))?;
    if new.definition_path() != old.definition_path()
        || !signature(new.result(), old.result(), meter, &path.clone().field(6))?
        || !signatures(
            new.type_parameters().arguments(),
            old.type_parameters().arguments(),
            meter,
            &path.clone().field(8),
        )?
    {
        return Ok(false);
    }
    origin(new.definition_origin(), meter, &path.clone().field(12))?;
    origin(old.definition_origin(), meter, &path.clone().field(12))?;
    if new.definition_origin() != old.definition_origin() {
        return Ok(false);
    }
    let new_max = locals::charge(new.locals(), meter, &path.clone().field(4))?;
    let old_max = locals::charge(old.locals(), meter, &path.clone().field(4))?;
    if new.locals() != old.locals() {
        return Ok(false);
    }
    if !optional_signature(
        new.receiver()
            .receiver()
            .map(TemplateReceiverV1::value_type),
        old.receiver()
            .receiver()
            .map(TemplateReceiverV1::value_type),
        meter,
        &path.clone().field(9),
    )? {
        return Ok(false);
    }
    sequence(
        new.value_parameters().parameters().len(),
        meter,
        &path.clone().field(10),
    )?;
    sequence(
        old.value_parameters().parameters().len(),
        meter,
        &path.clone().field(10),
    )?;
    if new.value_parameters() != old.value_parameters() {
        return Ok(false);
    }
    body::charge(
        new.body(),
        new.locals(),
        new.definition_origin(),
        new_max,
        meter,
        &path.clone().field(5),
    )?;
    body::charge(
        old.body(),
        old.locals(),
        old.definition_origin(),
        old_max,
        meter,
        &path.clone().field(5),
    )?;
    // Reference records have different authorities and are validated separately.
    // All shared body nodes and leaves have paid for this structural comparison.
    Ok(new.body() == old.body())
}

fn structural_path(
    value: &StructuralDefinitionPath,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<u64, WireError> {
    let count = value.segments().len() as u64;
    meter.check_semantic_depth(count.checked_add(1).ok_or_else(|| overflow(path))?, path)?;
    sequence(value.segments().len(), meter, path)?;
    let cost = count
        .checked_mul(16)
        .and_then(|n| n.checked_add(16))
        .ok_or_else(|| overflow(path))?;
    meter.charge_work(cost, path)?;
    Ok(cost)
}
fn selector(
    value: &LocalValueSelector,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<u64, WireError> {
    match value {
        LocalValueSelector::This | LocalValueSelector::Parameter { .. } => {
            meter.charge_work(16, path)?;
            Ok(16)
        }
        LocalValueSelector::LocalDeclaration { path: value }
        | LocalValueSelector::BoundReceiver { path: value }
        | LocalValueSelector::SuspensionResult { site: value }
        | LocalValueSelector::Synthetic { path: value, .. } => structural_path(value, meter, path),
    }
}
