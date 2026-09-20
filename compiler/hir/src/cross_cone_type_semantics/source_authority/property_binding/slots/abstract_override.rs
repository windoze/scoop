use super::*;
use scoop_identity::{DefinitionOwnerAtom, DispatchSlotKey};

pub(super) fn matches(
    dispatch: &BoundInheritanceDispatchSourcesV1<'_, '_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    owner: PersistentExactTypeId,
    record: &NominalSupportPropertyInterfaceV1,
    slot: &DispatchSlotKey,
    meter: &mut BudgetMeter,
) -> Result<bool, InheritancePropertyBindingError> {
    use InheritancePropertyBindingError as Error;
    let payload = payload(record)?;
    if payload.representation() != PropertyRepresentationV1::AbstractSlot {
        return Ok(false);
    }
    let DispatchDeclarationOwner::Accessor(root) = slot.owner() else {
        return Ok(false);
    };
    query(
        dispatch
            .foundation
            .source()
            .entries()
            .accessor_keys
            .values()
            .len(),
        meter,
    )?;
    let accessor = dispatch.accessor_key(root)?;
    if accessor.role() == AccessorRole::Setter
        && matches!(
            payload.mutability(),
            ProtectedPropertyMutabilityV1::ReadOnly
        )
    {
        return Ok(false);
    }
    let PropertyOwner::Property(root_property) = accessor.owner() else {
        return Err(Error::Accessor(root));
    };
    query(dispatch.properties.len(), meter)?;
    let root_key = dispatch.property_key(root_property)?;
    query(dispatch.properties.len(), meter)?;
    let own_key = dispatch.property_key(record.declaration())?;
    NominalRepresentationSupportV1::charge_source_key_resources(
        root_key,
        meter,
        &WirePath::root(),
    )?;
    NominalRepresentationSupportV1::charge_source_key_resources(own_key, meter, &WirePath::root())?;
    if root_key.name() != own_key.name() {
        return Ok(false);
    }
    let Some(DefinitionOwnerAtom::Type(root_owner)) = root_key.owners().owners().last() else {
        return Err(Error::Owner(root_property));
    };
    let root_owner = exact_owner(SourceNominalId::Concrete(*root_owner), meter)?;
    if !graph
        .is_subclass(owner, root_owner, meter)
        .map_err(AccessDomainSemanticError::Inheritance)?
    {
        return Ok(false);
    }
    let declaration = match accessor.role() {
        AccessorRole::Getter => InheritanceCallableDeclarationV1::Getter(root),
        AccessorRole::Setter => InheritanceCallableDeclarationV1::Setter(root),
    };
    query(dispatch.callables.records().len(), meter)?;
    let source = dispatch.callable(declaration)?;
    let signature = source.signature.exact_signature();
    let value = match accessor.role() {
        AccessorRole::Getter if signature.parameters().is_empty() => signature.result(),
        AccessorRole::Setter => {
            let [value] = signature.parameters() else {
                return Err(Error::Signature(record.declaration()));
            };
            *value
        }
        AccessorRole::Getter => return Err(Error::Signature(record.declaration())),
    };
    contracts::match_value_type(
        dispatch.foundation,
        record.declaration(),
        payload.value_type(),
        value,
        meter,
    )?;
    Ok(true)
}
