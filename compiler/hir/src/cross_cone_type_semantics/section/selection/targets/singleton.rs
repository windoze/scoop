use super::*;
use scoop_identity::PersistentObjectValueId;

pub(super) fn validate<F: TypeSectionFoundationSemanticAuthority<E>, E>(
    provider: &Exports<'_>,
    exact: PersistentExactTypeId,
    value: PersistentObjectValueId,
    foundation: &F,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error<E>> {
    meter.charge_work(3, path)?;
    let node = provider.graph.get(exact).ok_or(Error::Object)?;
    let SourceNominalId::Concrete(owner) = node.source() else {
        return Err(Error::Object);
    };
    let representation = provider.representations.get(owner).ok_or(Error::Object)?;
    if !matches!(
        representation.shape(),
        NominalRepresentationShapeV1::Object { .. }
    ) || provider.graph.object_backing_relation(exact).is_none()
    {
        return Err(Error::Object);
    }
    let key = foundation
        .nominal_declaration_key(node.source())
        .map_err(Error::Source)?;
    // Source key text and owner chains are charged before identity hashing.
    NominalRepresentationSupportV1::charge_source_key_resources(key, meter, path)?;
    let bytes = scoop_wire::encoded_length(key).map_err(Error::Encoding)?;
    meter.charge_sha256(bytes.saturating_add(192), path)?;
    if PersistentObjectValueId::from_source_object(key).ok() != Some(value) {
        return Err(Error::Object);
    }
    Ok(())
}
