use super::*;

pub(super) fn project<'a>(
    data: &mut Data<'a>,
    declaration: Declaration,
    dependencies: &[CheckedSharedTypeFoundationV1<'a>],
) -> Result<(), Error> {
    if data.signatures.contains_key(&declaration) {
        return Ok(());
    }

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
    scoop_wire::allocation::try_reserve(
        &mut parameters,
        source_parameters.len(),
        &WirePath::root(),
    )?;
    for parameter in source_parameters {
        parameters.push(types.exact(parameter.value_type())?);
    }
    let result = types.exact(member.source.result())?;
    for exact in parameters.iter().copied().chain([result, member.owner]) {
        exact_keys(&mut data.exacts, types, exact)?;
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

    data.signatures.insert(declaration, signature);
    Ok(())
}

fn exact_keys(
    keys: &mut BTreeMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    types: MetadataTypes<'_, '_>,
    exact: PersistentExactTypeId,
) -> Result<(), Error> {
    if keys.contains_key(&exact) {
        return Ok(());
    }
    let key = types.key(exact)?;

    for child in key.exact_type_dependencies() {
        exact_keys(keys, types, child)?;
    }
    keys.insert(exact, key);
    Ok(())
}
