use super::*;

pub(super) fn project<'a>(
    data: &mut Data<'a>,
    declaration: Declaration,
    dependencies: &[CheckedSharedTypeFoundationV1<'a>],
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    contracts::lookup(data.signatures.len(), meter)?;
    if data.signatures.contains_key(&declaration) {
        return Ok(());
    }
    contracts::lookup(data.members.len(), meter)?;
    let member = data
        .members
        .get(&declaration)
        .ok_or(Error::SlotCallable(declaration))?;
    let types = MetadataTypes {
        current: member.metadata,
        dependencies,
    };
    let mut parameters = Vec::new();
    let source_parameters = member.source.parameters().parameters();
    meter.try_reserve_collection_slots(
        &mut parameters,
        source_parameters.len(),
        &WirePath::root(),
    )?;
    for parameter in source_parameters {
        parameters.push(types.exact(parameter.value_type(), 1, meter)?);
    }
    let result = types.exact(member.source.result(), 1, meter)?;
    for exact in parameters.iter().copied().chain([result, member.owner]) {
        exact_keys(&mut data.exacts, types, exact, 1, meter)?;
    }
    let signature = InheritanceCallableSignatureV1::try_new(
        ExactCallableSignature::new(
            member.source.effects().execution(),
            Some(member.owner),
            parameters,
            result,
        ),
        member.source.effects(),
    )
    .map_err(|_| Error::SlotCallable(declaration))?;
    meter.charge_collection_slots(1, &WirePath::root())?;
    data.signatures.insert(declaration, signature);
    Ok(())
}

fn exact_keys(
    keys: &mut BTreeMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    types: MetadataTypes<'_, '_>,
    exact: PersistentExactTypeId,
    depth: u64,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    meter.check_semantic_depth(depth, &WirePath::root())?;
    contracts::lookup(keys.len(), meter)?;
    if keys.contains_key(&exact) {
        return Ok(());
    }
    let key = types.key(exact, meter)?;
    let count = match key.as_ref() {
        ExactTypeKey::Nominal(_) => 0,
        ExactTypeKey::NominalApplication { arguments, .. } | ExactTypeKey::Tuple(arguments) => {
            arguments.as_slice().len()
        }
        ExactTypeKey::Function { parameters, .. }
        | ExactTypeKey::NativeFunctionPointer { parameters, .. } => parameters.len() + 1,
        ExactTypeKey::RawPointer(_) => 1,
    };
    meter.charge_collection_slots(count as u64 + 1, &WirePath::root())?;
    meter.charge_edges(count as u64, &WirePath::root())?;
    for child in key.exact_type_dependencies() {
        exact_keys(keys, types, child, depth + 1, meter)?;
    }
    keys.insert(exact, key);
    Ok(())
}
