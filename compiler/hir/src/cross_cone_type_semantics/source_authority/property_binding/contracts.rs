use super::*;

pub(super) fn validate(
    dispatch: &BoundInheritanceDispatchSourcesV1<'_, '_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    key: &SourceDeclarationKey,
    record: &NominalSupportPropertyInterfaceV1,
) -> Result<(), InheritancePropertyBindingError> {
    use InheritancePropertyBindingError as Error;
    let property = record.declaration();
    let payload = payload(record)?;
    let owner = exact_owner(payload.owner())?;

    let source = dispatch
        .inventory()
        .get(owner)
        .ok_or(Error::Owner(property))?;
    let getter = access::validate(
        dispatch.foundation,
        property,
        key,
        DefinitionOriginSubject::Property(property),
        record.declaration_access(),
    )?;
    access::accessor(
        dispatch.foundation,
        property,
        payload.getter(),
        AccessorRole::Getter,
    )?;
    if let ProtectedPropertyMutabilityV1::ReadWrite {
        setter,
        setter_access,
    } = payload.mutability()
    {
        access::accessor(dispatch.foundation, property, *setter, AccessorRole::Setter)?;
        let setter = access::validate(
            dispatch.foundation,
            property,
            key,
            DefinitionOriginSubject::PropertyAccessor(*setter),
            setter_access,
        )?;
        let getter_domain = graph.replay_declaration_access(getter)?;
        let setter_domain = graph.replay_declaration_access(setter)?;
        if !getter_domain.lookup().covers(setter_domain.lookup())? {
            return Err(Error::SetterDomain(property));
        }
    }
    for declaration in std::iter::once(InheritanceCallableDeclarationV1::Getter(payload.getter()))
        .chain(match payload.mutability() {
            ProtectedPropertyMutabilityV1::ReadOnly => None,
            ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } => {
                Some(InheritanceCallableDeclarationV1::Setter(*setter))
            }
        })
    {
        if let Some(callable) = dispatch.callables.get(declaration) {
            signature(dispatch.foundation, owner, record, callable)?;
        }
    }
    slots::validate(dispatch, graph, source, record)
}

fn signature(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    owner: PersistentExactTypeId,
    record: &NominalSupportPropertyInterfaceV1,
    callable: &InheritanceSourceCallableV1,
) -> Result<(), InheritancePropertyBindingError> {
    use InheritancePropertyBindingError as Error;
    let property = record.declaration();
    let payload = payload(record)?;
    let signature = callable.signature().exact_signature();
    if signature.receiver().into_option() != Some(owner) {
        return Err(Error::Signature(property));
    }
    let (exact_value, access) = match callable.declaration() {
        InheritanceCallableDeclarationV1::Getter(_) if signature.parameters().is_empty() => {
            (signature.result(), record.declaration_access())
        }
        InheritanceCallableDeclarationV1::Setter(_) => {
            let [value] = signature.parameters() else {
                return Err(Error::Signature(property));
            };
            let ProtectedPropertyMutabilityV1::ReadWrite { setter_access, .. } =
                payload.mutability()
            else {
                return Err(Error::Signature(property));
            };
            (*value, setter_access)
        }
        _ => return Err(Error::Signature(property)),
    };

    if callable.declaration_access().declared_visibility() != access.declared_visibility()
        || callable.declaration_access().lexical_owners() != access.lexical_owners()
    {
        return Err(Error::Visibility(property));
    }
    match_value_type(foundation, property, payload.value_type(), exact_value)
}

pub(super) fn match_value_type(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    property: PersistentPropertyId,
    value: &scoop_identity::SignatureTypeKey,
    exact: PersistentExactTypeId,
) -> Result<(), InheritancePropertyBindingError> {
    use InheritancePropertyBindingError as Error;
    NominalRepresentationSupportV1::validate_exact_field_types(
        std::slice::from_ref(value),
        &[exact],
        foundation,
    )
    .map_err(|error| match error {
        InheritanceSlotContractSemanticError::Resource(error) => Error::Resource(error),
        InheritanceSlotContractSemanticError::Foundation(error) => error.into(),
        _ => Error::Signature(property),
    })
}
