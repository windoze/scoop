use super::*;

pub(super) fn project<'a>(
    data: &mut Data<'a>,
    metadata: SharedTypeMetadataV1<'a>,
    dependencies: &[SharedTypeMetadataV1<'a>],
    root: PersistentExactTypeId,
    declaration: Declaration,
    receiver: PersistentExactTypeId,
) -> Result<(), Error> {
    let member = data
        .members
        .get(&declaration)
        .ok_or(Error::SlotCallable(declaration))?;
    let owner = member
        .source
        .owner()
        .nominal_owner()
        .ok_or(Error::SlotCallable(declaration))?;
    let expected = metadata.applied_member_receiver(root, owner, dependencies)?;
    if receiver != expected {
        return Err(Error::SlotCallable(declaration));
    }
    if data.signatures.contains_key(&(declaration, receiver)) {
        return Ok(());
    }
    let signature = metadata.applied_member_signature(receiver, member.source)?;
    for exact in signature
        .exact_signature()
        .parameters()
        .iter()
        .copied()
        .chain([signature.exact_signature().result(), receiver])
        .chain(signature.context_keys().iter().map(|key| key.0))
    {
        exact_keys(&mut data.exacts, metadata, exact)?;
    }
    data.signatures.insert((declaration, receiver), signature);
    Ok(())
}

fn exact_keys(
    keys: &mut BTreeMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    metadata: SharedTypeMetadataV1<'_>,
    exact: PersistentExactTypeId,
) -> Result<(), Error> {
    if keys.contains_key(&exact) {
        return Ok(());
    }
    let key = metadata
        .identities
        .canonical_key::<_, ExactTypeKey>(exact)?;
    for child in key.exact_type_dependencies() {
        exact_keys(keys, metadata, child)?;
    }
    keys.insert(exact, key);
    Ok(())
}
