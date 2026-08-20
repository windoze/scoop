use super::*;
use scoop_identity::{
    CallableOwner, CallableTemplateOwner, DefinitionOriginSubject, DefinitionOwnerAtom,
    NominalDeclarationOwner, SourceContextKey,
};

pub(super) fn local(
    key: &SourceDeclarationKey,
    identity: Identity,
) -> Result<CallableTemplateOwner, Error> {
    Ok(match key.owners().owners().last() {
        Some(DefinitionOwnerAtom::Function(id)) => CallableTemplateOwner::Function(*id),
        Some(DefinitionOwnerAtom::GenericFunction(id)) => {
            CallableTemplateOwner::GenericFunction(*id)
        }
        Some(DefinitionOwnerAtom::Constructor(id)) => CallableTemplateOwner::Constructor(*id),
        Some(DefinitionOwnerAtom::PropertyAccessor(id)) => CallableTemplateOwner::Accessor(*id),
        Some(DefinitionOwnerAtom::GeneratedCallable(id)) => CallableTemplateOwner::Generated(*id),
        Some(DefinitionOwnerAtom::EnumVariant(id)) => {
            CallableTemplateOwner::VariantConstructor(*id)
        }
        _ => return Err(failure(identity, Failure::LexicalParent)),
    })
}

pub(super) fn validate(
    foundation: &NestedIdentityInput<'_>,
    parent: CallableTemplateOwner,
    identity: Identity,
    origin: &ExportDefinitionSourceV1,
) -> Result<(), Error> {
    let context = foundation
        .foundation
        .source_context_key(origin.origin().context())
        .ok_or_else(|| failure(identity, Failure::DefinitionContext))?;
    let expected = match parent {
        CallableTemplateOwner::Function(id) => CallableOwner::Function(id),
        CallableTemplateOwner::GenericFunction(id) => CallableOwner::GenericTemplate(id),
        CallableTemplateOwner::Constructor(id) => CallableOwner::Constructor(id),
        CallableTemplateOwner::Accessor(id) => CallableOwner::Accessor(id),
        CallableTemplateOwner::Generated(id) => CallableOwner::Generated(id),
        CallableTemplateOwner::VariantConstructor(id) => {
            return variant(foundation, id, identity, origin, context);
        }
        CallableTemplateOwner::ReleaseHook(_) => {
            return Err(failure(identity, Failure::LexicalParent));
        }
    };

    if !matches!(context, SourceContextKey::Callable { owner, .. } if *owner == expected) {
        return Err(failure(identity, Failure::DefinitionContext));
    }
    Ok(())
}

fn variant(
    foundation: &NestedIdentityInput<'_>,
    variant: scoop_identity::PersistentEnumVariantId,
    identity: Identity,
    origin: &ExportDefinitionSourceV1,
    context: &SourceContextKey,
) -> Result<(), Error> {
    let canonical = foundation.foundation;
    let key = key(
        canonical.type_source_enum_variant_records(),
        variant,
        identity,
    )?;
    let expected = match key
        .source_owner()
        .ok_or_else(|| failure(identity, Failure::LexicalParent))?
    {
        SourceNominalId::Concrete(id) => NominalDeclarationOwner::Concrete(id),
        SourceNominalId::GenericTemplate(id) => NominalDeclarationOwner::GenericTemplate(id),
    };

    if !matches!(context, SourceContextKey::Nominal { owner, .. } if *owner == expected) {
        return Err(failure(identity, Failure::DefinitionContext));
    }

    let declaration = foundation
        .foundation
        .definition_origin(DefinitionOriginSubject::EnumVariant(variant))
        .ok_or_else(|| failure(identity, Failure::LexicalParent))?
        .origin();

    let outer = declaration.span();
    let inner = origin.origin().span();
    if declaration.source() != origin.origin().source()
        || inner.start_byte() < outer.start_byte()
        || inner.end_byte() > outer.end_byte()
    {
        return Err(failure(identity, Failure::DefinitionContext));
    }
    Ok(())
}
