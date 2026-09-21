use super::*;
use crate::cross_cone_type_semantics::source_authority::binding_keys;
use scoop_identity::{
    CallableOwner, CborIdentityKey, CborIdentityRecord, DefinitionOriginSubject,
    NominalDeclarationOwner, PersistentId, SourceContextKey,
};

pub(super) fn validate(
    provider: &BoundTypeFoundationSourcesV1<'_>,
    template: &DefaultSourceTemplateV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error> {
    let root = template.definition_root();
    let canonical = provider.foundation.as_canonical();
    let (subject, expected_context) = match root {
        PersistentLexicalRootV1::Function(id) => {
            let key = key(
                provider,
                canonical.type_source_function_records(),
                id,
                root,
                meter,
                path,
            )?;
            if key.origin() != provider.source().entries().provider {
                return Err(Error::RootProvider(root));
            }
            (
                DefinitionOriginSubject::Function(id),
                RootContext::Callable(CallableOwner::Function(id)),
            )
        }
        PersistentLexicalRootV1::GenericFunction(id) => {
            let key = key(
                provider,
                canonical.type_source_generic_function_records(),
                id,
                root,
                meter,
                path,
            )?;
            if key.origin() != provider.source().entries().provider {
                return Err(Error::RootProvider(root));
            }
            (
                DefinitionOriginSubject::GenericFunction(id),
                RootContext::Callable(CallableOwner::GenericTemplate(id)),
            )
        }
        PersistentLexicalRootV1::Constructor(id) => {
            let key = key(
                provider,
                canonical.type_source_constructor_records(),
                id,
                root,
                meter,
                path,
            )?;
            if key.origin() != provider.source().entries().provider {
                return Err(Error::RootProvider(root));
            }
            (
                DefinitionOriginSubject::Constructor(id),
                RootContext::Callable(CallableOwner::Constructor(id)),
            )
        }
        PersistentLexicalRootV1::EnumVariantConstructor(id) => {
            let key = key(
                provider,
                canonical.type_source_enum_variant_records(),
                id,
                root,
                meter,
                path,
            )?;
            let owner = key.source_owner().ok_or(Error::RootProvider(root))?;
            meter.charge_work(
                u64::from(
                    provider
                        .source()
                        .entries()
                        .sources
                        .records()
                        .len()
                        .max(1)
                        .ilog2(),
                ) + 1,
                path,
            )?;
            if provider.nominal_key(owner)?.origin() != provider.source().entries().provider {
                return Err(Error::RootProvider(root));
            }
            (
                DefinitionOriginSubject::EnumVariant(id),
                RootContext::Nominal(match owner {
                    SourceNominalId::Concrete(id) => NominalDeclarationOwner::Concrete(id),
                    SourceNominalId::GenericTemplate(id) => {
                        NominalDeclarationOwner::GenericTemplate(id)
                    }
                }),
            )
        }
    };
    meter.charge_work(
        u64::from(canonical.counts().definition_origins.max(1).ilog2()) + 1,
        path,
    )?;
    let declaration = provider
        .foundation
        .definition_origin(subject)
        .ok_or(Error::RootOrigin(root))?
        .origin();
    let body = template.definition_origin().origin();
    let bytes = (declaration.source().logical_path().as_str().len() as u64)
        .saturating_add(body.source().logical_path().as_str().len() as u64);
    meter.check_semantic_leaf(bytes, path)?;
    meter.charge_work(bytes.saturating_add(64), path)?;
    meter.charge_work(
        u64::from(canonical.counts().source_contexts.max(1).ilog2()) + 1,
        path,
    )?;
    let context = provider
        .foundation
        .source_context_key(body.context())
        .ok_or(Error::RootOrigin(root))?;
    if declaration.source() != body.source() || !expected_context.matches(context) {
        return Err(Error::RootOrigin(root));
    }
    Ok(())
}

fn key<'k, I, K>(
    provider: &BoundTypeFoundationSourcesV1<'_>,
    records: &'k [CborIdentityRecord<I, K>],
    id: I,
    root: PersistentLexicalRootV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<&'k K, Error>
where
    I: PersistentId + 'static,
    K: CborIdentityKey<I> + Eq + Clone + Send + Sync + 'static,
{
    // Declaration tables preserve dependency order, rather than ID order.
    meter.check_table_entries(records.len() as u64, path)?;
    meter.charge_work((records.len() as u64).saturating_mul(64), path)?;
    let record = records
        .iter()
        .find(|record| record.id() == id)
        .ok_or(Error::MissingRoot(root))?;
    binding_keys::verify(id, record.key(), provider.identities, meter, path)?;
    Ok(record.key())
}

#[derive(Clone, Copy)]
enum RootContext {
    Callable(CallableOwner),
    Nominal(NominalDeclarationOwner),
}
impl RootContext {
    fn matches(self, context: &SourceContextKey) -> bool {
        match (self, context) {
            (Self::Callable(expected), SourceContextKey::Callable { owner, .. }) => {
                expected == *owner
            }
            (Self::Nominal(expected), SourceContextKey::Nominal { owner, .. }) => {
                expected == *owner
            }
            _ => false,
        }
    }
}
