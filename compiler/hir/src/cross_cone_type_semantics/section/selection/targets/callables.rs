use super::*;
use scoop_identity::{AccessorRole, PersistentPropertyAccessorId};
mod accessors;

pub(super) fn member<F: TypeSectionFoundationSemanticAuthority<E>, E>(
    provider: &Exports<'_>,
    declaration: InheritanceCallableDeclarationV1,
    foundation: &F,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<PersistentExactTypeId, Error<E>> {
    let declaration = match declaration {
        InheritanceCallableDeclarationV1::Function(id) => CallableTemplateOrigin::Function(id),
        InheritanceCallableDeclarationV1::Getter(id) => {
            accessor_role(id, AccessorRole::Getter, foundation, meter, path)?;
            CallableTemplateOrigin::Accessor(id)
        }
        InheritanceCallableDeclarationV1::Setter(id) => {
            accessor_role(id, AccessorRole::Setter, foundation, meter, path)?;
            CallableTemplateOrigin::Accessor(id)
        }
    };
    exact(
        source_owner(provider, declaration, meter, path)?,
        meter,
        path,
    )
}

pub(super) fn construction<F: TypeSectionFoundationSemanticAuthority<E>, E>(
    provider: &Exports<'_>,
    owner: PersistentExactTypeId,
    declaration: SelectedTypeConstructionV1,
    foundation: &F,
    meter: &mut BudgetMeter,
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
    if exact(
        source_owner(provider, declaration, meter, path)?,
        meter,
        path,
    )? != owner
    {
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
            meter.check_table_entries(variants.len() as u64, path)?;
            meter.charge_work(variants.len() as u64, path)?;
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
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<SourceNominalId, Error<E>> {
    let public = provider.public.section().callable_interfaces();
    meter.charge_work(
        (public.records().len() as u64 + provider.sources.entries().len() as u64 + 2).ilog2()
            as u64
            + 2,
        path,
    )?;
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
        accessors::find(provider, id, meter, path)?
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
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error<E>> {
    let key = foundation
        .selected_accessor_key(id)
        .map_err(Error::Source)?;
    // This key has only a typed property ID and a finite role tag.
    let bytes = scoop_wire::encoded_length(key).map_err(Error::Encoding)?;
    meter.charge_sha256(bytes.saturating_add(64), path)?;
    if key.role() != role || PersistentPropertyAccessorId::from_key(key).ok() != Some(id) {
        return Err(Error::DeclarationRole);
    }
    Ok(())
}
