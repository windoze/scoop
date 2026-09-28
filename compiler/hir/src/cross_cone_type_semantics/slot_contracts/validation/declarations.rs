use scoop_identity::{
    AccessorRole, DefinitionOwnerAtom, DispatchSlotKey, DuplicateSignatureKey,
    PersistentFunctionId, PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId,
    PropertyOwner, SourceDeclarationKind,
};

use super::{
    Declaration, InheritanceSlotContractSemanticAuthority,
    InheritanceSlotContractSemanticError as Error, types,
};
use crate::{
    CheckedNominalInheritanceGraphV1, DeclarationAccessSourceV1, DeclaredVisibilityV1,
    InheritanceCallableDeclarationV1 as Decl, InheritanceCallableSignatureV1, SourceNominalId,
};

pub(super) fn validate<'s, A: InheritanceSlotContractSemanticAuthority<E>, E>(
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    declaration: Decl,
    owner: PersistentTypeId,
    signature: &InheritanceCallableSignatureV1,
    access: &DeclarationAccessSourceV1,
    authority: &'s A,
) -> Result<Declaration<'s>, Error<E>> {
    let key = match declaration {
        Decl::Function(id) => {
            let key = authority.function_key(id).map_err(Error::Foundation)?;
            if PersistentFunctionId::from_source_declaration(key).ok() != Some(id) {
                return Err(Error::DeclarationIdentity(declaration));
            }
            key
        }
        Decl::Getter(id) | Decl::Setter(id) => {
            let accessor = authority.accessor_key(id).map_err(Error::Foundation)?;
            let expected = if matches!(declaration, Decl::Getter(_)) {
                AccessorRole::Getter
            } else {
                AccessorRole::Setter
            };
            if PersistentPropertyAccessorId::from_key(accessor).ok() != Some(id)
                || accessor.role() != expected
            {
                return Err(Error::DeclarationIdentity(declaration));
            }
            let PropertyOwner::Property(property) = accessor.owner() else {
                return Err(Error::DeclarationIdentity(declaration));
            };
            let key = authority
                .property_key(property)
                .map_err(Error::Foundation)?;
            if PersistentPropertyId::from_source_declaration(key).ok() != Some(property) {
                return Err(Error::DeclarationIdentity(declaration));
            }
            key
        }
    };
    if key.duplicate_signature().type_parameter_count() != 0
        || key.duplicate_signature().receiver_is_present()
        || key.owners().owners().last() != Some(&DefinitionOwnerAtom::Type(owner))
    {
        return Err(Error::DeclarationIdentity(declaration));
    }
    let owner_source = graph
        .source(SourceNominalId::Concrete(owner))
        .ok_or(Error::DeclarationIdentity(declaration))?;
    if key.origin() != owner_source.key.origin()
        || key.package() != owner_source.key.package()
        || key.owners().owners().split_last().map(|(_, outer)| outer)
            != Some(owner_source.key.owners().owners())
    {
        return Err(Error::DeclarationIdentity(declaration));
    }
    let exact_owner = graph
        .source_exact(SourceNominalId::Concrete(owner))
        .map_err(Error::Inheritance)?;
    if signature.exact_signature().receiver().into_option() != Some(exact_owner) {
        return Err(Error::ReceiverOwner);
    }
    match (declaration, key.duplicate_signature()) {
        (Decl::Function(_), DuplicateSignatureKey::Function { parameters, .. }) => {
            types::match_parameters(
                parameters,
                signature.exact_signature().parameters(),
                authority,
            )?;
        }
        (Decl::Getter(_), DuplicateSignatureKey::Property { .. })
            if signature.exact_signature().parameters().is_empty() => {}
        (Decl::Setter(_), DuplicateSignatureKey::Property { .. })
            if signature.exact_signature().parameters().len() == 1
                && signature.exact_signature().result()
                    == authority.unit_exact_type().map_err(Error::Foundation)? => {}
        _ => return Err(Error::Signature),
    }
    for exact in signature
        .exact_signature()
        .parameters()
        .iter()
        .chain(std::iter::once(&signature.exact_signature().result()))
    {
        types::validate_exact_identity(*exact, authority)?;
    }
    if access.declared_visibility() == DeclaredVisibilityV1::Private {
        return Err(Error::PrivateDeclaration);
    }
    graph
        .check_declaration_source(access, key, authority)
        .map_err(Error::Source)?;
    Ok(Declaration { key, exact_owner })
}

pub(super) fn root_key(declaration: Decl, kind: SourceDeclarationKind) -> Option<DispatchSlotKey> {
    if !matches!(
        kind,
        SourceDeclarationKind::Class | SourceDeclarationKind::Interface
    ) {
        return None;
    }
    Some(match declaration {
        Decl::Function(id) if kind == SourceDeclarationKind::Class => {
            DispatchSlotKey::virtual_method(id)
        }
        Decl::Function(id) => DispatchSlotKey::interface_method(id),
        Decl::Getter(id) => DispatchSlotKey::property_getter(id),
        Decl::Setter(id) => DispatchSlotKey::property_setter(id),
    })
}
