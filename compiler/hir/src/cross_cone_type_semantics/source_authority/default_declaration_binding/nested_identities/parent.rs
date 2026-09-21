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
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    parent: CallableTemplateOwner,
    identity: Identity,
    origin: &ExportDefinitionSourceV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error> {
    let canonical = foundation.foundation.as_canonical();
    sources::query(canonical.counts().source_contexts, meter, path)?;
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
            return variant(foundation, id, identity, origin, context, meter, path);
        }
    };
    meter.charge_work(65, path)?;
    if !matches!(context, SourceContextKey::Callable { owner, .. } if *owner == expected) {
        return Err(failure(identity, Failure::DefinitionContext));
    }
    Ok(())
}

fn variant(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    variant: scoop_identity::PersistentEnumVariantId,
    identity: Identity,
    origin: &ExportDefinitionSourceV1,
    context: &SourceContextKey,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error> {
    let canonical = foundation.foundation.as_canonical();
    let key = key(
        foundation,
        canonical.type_source_enum_variant_records(),
        variant,
        identity,
        meter,
        path,
    )?;
    let expected = match key
        .source_owner()
        .ok_or_else(|| failure(identity, Failure::LexicalParent))?
    {
        SourceNominalId::Concrete(id) => NominalDeclarationOwner::Concrete(id),
        SourceNominalId::GenericTemplate(id) => NominalDeclarationOwner::GenericTemplate(id),
    };
    meter.charge_work(65, path)?;
    if !matches!(context, SourceContextKey::Nominal { owner, .. } if *owner == expected) {
        return Err(failure(identity, Failure::DefinitionContext));
    }
    sources::query(canonical.counts().definition_origins, meter, path)?;
    let declaration = foundation
        .foundation
        .definition_origin(DefinitionOriginSubject::EnumVariant(variant))
        .ok_or_else(|| failure(identity, Failure::LexicalParent))?
        .origin();
    let bytes = declaration
        .source()
        .logical_path()
        .as_str()
        .len()
        .saturating_add(origin.origin().source().logical_path().as_str().len())
        as u64;
    meter.charge_work(bytes.saturating_add(66), path)?;
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
