use super::*;
use scoop_wire::{BudgetMeter, WireError, WireErrorKind};

pub(super) fn mapping(
    arguments: Vec<SignatureTypeKey>,
    path: &WirePath,
) -> Result<CanonicalBinderUseListV1, Error> {
    CanonicalBinderUseListV1::try_new(arguments).map_err(|_| count_error(path))
}

pub(super) fn shape(arity: u32, path: &WirePath) -> Result<DefaultTemplateProviderShapeV1, Error> {
    DefaultTemplateProviderShapeV1::try_new(arity, 0).map_err(|_| count_error(path))
}

fn count_error(path: &WirePath) -> Error {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None).into()
}

pub(super) fn copy_arguments(
    arguments: &[SignatureTypeKey],
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<CanonicalBinderUseListV1, Error> {
    let mut output = Vec::new();
    meter.try_reserve_collection_slots(&mut output, arguments.len(), path)?;
    for argument in arguments {
        output.push(scoop_hir::copy_default_signature_type_metered(
            argument, meter, path,
        )?);
    }
    mapping(output, path)
}

pub(super) fn equal_types(
    left: &SignatureTypeKey,
    right: &SignatureTypeKey,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, Error> {
    let cost = scoop_wire::encoded_length(left)
        .and_then(|left| scoop_wire::encoded_length(right).map(|right| left.saturating_add(right)))
        .map_err(|e| Error::Encoding(e.to_string()))?;
    meter.charge_work(cost, path)?;
    Ok(left == right)
}

pub(super) fn equal_arguments(
    left: &CanonicalBinderUseListV1,
    right: &CanonicalBinderUseListV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, Error> {
    meter.charge_work(1, path)?;
    if left.len_u32() != right.len_u32() {
        return Ok(false);
    }
    for (left, right) in left.arguments().iter().zip(right.arguments()) {
        if !equal_types(left, right, meter, path)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn matches(
    source: Source<'_>,
    inherited: Source<'_>,
    arguments: &CanonicalBinderUseListV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, Error> {
    let current = source.declaration;
    let candidate = inherited.declaration;
    meter.charge_work(1, path)?;
    if source.key.name() != inherited.key.name()
        || current.type_parameters().len_u32() != candidate.type_parameters().len_u32()
        || current.parameters().parameters().len() != candidate.parameters().parameters().len()
    {
        return Ok(false);
    }
    let shape = shape(arguments.len_u32(), path)?;
    let current_types = current
        .parameters()
        .parameters()
        .iter()
        .map(|parameter| parameter.value_type())
        .chain(std::iter::once(current.result()));
    let inherited_types = candidate
        .parameters()
        .parameters()
        .iter()
        .map(|parameter| parameter.value_type())
        .chain(std::iter::once(candidate.result()));
    for (current, inherited) in current_types.zip(inherited_types) {
        let applied = arguments.substitute_provider_type_metered(shape, inherited, meter, path)?;
        if !equal_types(current, &applied, meter, path)? {
            return Ok(false);
        }
    }
    let current_effects = current.effects();
    let inherited_effects = candidate.effects();
    if current_effects.execution() != inherited_effects.execution()
        || current_effects.operator_role() != inherited_effects.operator_role()
        || current_effects.infix() != inherited_effects.infix()
    {
        return Err(Error::SignatureEffects(candidate.declaration()));
    }
    Ok(true)
}
