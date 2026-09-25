use super::*;
use scoop_wire::{WireError, WireErrorKind};

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

    path: &WirePath,
) -> Result<CanonicalBinderUseListV1, Error> {
    let mut output = Vec::new();
    scoop_wire::allocation::try_reserve(&mut output, arguments.len(), path)?;
    for argument in arguments {
        output.push(scoop_hir::copy_default_signature_type(argument, path)?);
    }
    mapping(output, path)
}

pub(super) fn equal_types(left: &SignatureTypeKey, right: &SignatureTypeKey) -> bool {
    left == right
}

pub(super) fn equal_arguments(
    left: &CanonicalBinderUseListV1,
    right: &CanonicalBinderUseListV1,
) -> bool {
    if left.len_u32() != right.len_u32() {
        return false;
    }
    for (left, right) in left.arguments().iter().zip(right.arguments()) {
        if !equal_types(left, right) {
            return false;
        }
    }
    true
}

pub(super) fn matches(
    source: Source<'_>,
    inherited: Source<'_>,
    arguments: &CanonicalBinderUseListV1,

    path: &WirePath,
) -> Result<bool, Error> {
    let current = source.declaration;
    let candidate = inherited.declaration;

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
        let applied = arguments.substitute_provider_type(shape, inherited)?;
        if !equal_types(current, &applied) {
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
