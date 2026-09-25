use super::*;
use scoop_identity::{AccessorRole, PersistentPropertyAccessorId};
mod accessors;

pub(super) fn member<F: TypeSectionFoundationSemanticAuthority<E>, E>(
    provider: &Exports<'_>,
    declaration: InheritanceCallableDeclarationV1,
    foundation: &F,

    path: &WirePath,
) -> Result<PersistentExactTypeId, Error<E>> {
    let declaration = match declaration {
        InheritanceCallableDeclarationV1::Function(id) => CallableTemplateOrigin::Function(id),
        InheritanceCallableDeclarationV1::Getter(id) => {
            accessor_role(id, AccessorRole::Getter, foundation)?;
            CallableTemplateOrigin::Accessor(id)
        }
        InheritanceCallableDeclarationV1::Setter(id) => {
            accessor_role(id, AccessorRole::Setter, foundation)?;
            CallableTemplateOrigin::Accessor(id)
        }
    };
    exact(source_owner(provider, declaration, path)?)
}

pub(super) fn construction<F: TypeSectionFoundationSemanticAuthority<E>, E>(
    provider: &Exports<'_>,
    owner: PersistentExactTypeId,
    declaration: SelectedTypeConstructionV1,
    foundation: &F,

    path: &WirePath,
) -> Result<(), Error<E>> {
    let (declaration, variant) = match declaration {
        SelectedTypeConstructionV1::Constructor(id) => {
            (CallableTemplateOrigin::Constructor(id), None)
        }
        SelectedTypeConstructionV1::EnumVariant(id) => {
            (CallableTemplateOrigin::VariantConstructor(id), Some(id))
        }
    };
    if exact(source_owner(provider, declaration, path)?)? != owner {
        return Err(Error::DeclarationOwner);
    }
    let key = foundation.exact_type_key(owner).map_err(Error::Source)?;
    let ExactTypeKey::Nominal(nominal) = key else {
        return Err(Error::RequiresOdr(owner));
    };
    let shape = provider
        .representations
        .get(*nominal)
        .ok_or(Error::DeclarationOwner)?
        .shape();
    match (variant, shape) {
        (Some(id), NominalRepresentationShapeV1::Enum { variants }) => {
            if variants.iter().any(|variant| variant.variant() == id) {
                Ok(())
            } else {
                Err(Error::DeclarationOwner)
            }
        }
        (
            None,
            NominalRepresentationShapeV1::Class { .. }
            | NominalRepresentationShapeV1::Struct { .. }
            | NominalRepresentationShapeV1::Intrinsic { .. },
        ) => Ok(()),
        _ => Err(Error::DeclarationOwner),
    }
}

fn source_owner<E>(
    provider: &Exports<'_>,
    declaration: CallableTemplateOrigin,

    path: &WirePath,
) -> Result<SourceNominalId, Error<E>> {
    let public = provider.public.section().callable_interfaces();

    let old = public.get(declaration);
    let new = provider.sources.get(declaration);
    let old_owner = if let Some(record) = old {
        if record.receiver().is_some() || !record.type_parameters().is_empty() {
            return Err(Error::DeclarationOwner);
        }
        Some(
            record
                .owner()
                .nominal_owner()
                .ok_or(Error::DeclarationOwner)?,
        )
    } else {
        None
    };
    let source = if let Some(record) = new {
        Some((record.payload(), record.declaration_access()))
    } else if let CallableTemplateOrigin::Accessor(id) = declaration {
        accessors::find(provider, id, path)?
    } else {
        None
    };
    let new_owner = if let Some((payload, access)) = source {
        if !payload.type_parameters().is_empty()
            || access.definition_origin().origin().source().cone() != provider.provider
        {
            return Err(Error::DeclarationOwner);
        }
        Some(payload.owner())
    } else {
        None
    };
    if let (Some(old), Some(new)) = (old_owner, new_owner)
        && old != new
    {
        return Err(Error::DeclarationOwner);
    }
    // Complete section overlap validation has compared every shared protocol,
    // effect and signature. Presence in either surface never grants access.
    old_owner.or(new_owner).ok_or(Error::DeclarationOwner)
}

fn accessor_role<F: TypeSectionFoundationSemanticAuthority<E>, E>(
    id: PersistentPropertyAccessorId,
    role: AccessorRole,
    foundation: &F,
) -> Result<(), Error<E>> {
    let key = foundation
        .selected_accessor_key(id)
        .map_err(Error::Source)?;
    // This key has only a typed property ID and a finite role tag.

    if key.role() != role || PersistentPropertyAccessorId::from_key(key).ok() != Some(id) {
        return Err(Error::DeclarationRole);
    }
    Ok(())
}
