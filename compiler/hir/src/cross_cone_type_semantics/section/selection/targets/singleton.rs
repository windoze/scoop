use super::*;
use scoop_identity::PersistentObjectValueId;

pub(super) fn validate<F: TypeSectionFoundationSemanticAuthority<E>, E>(
    provider: &Exports<'_>,
    exact: PersistentExactTypeId,
    value: PersistentObjectValueId,
    foundation: &F,
) -> Result<(), Error<E>> {
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

    if PersistentObjectValueId::from_source_object(key).ok() != Some(value) {
        return Err(Error::Object);
    }
    Ok(())
}
