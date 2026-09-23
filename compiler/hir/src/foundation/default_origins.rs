//! Default roots checked against their defining artifact's canonical records.

use scoop_identity::{
    CallableOwner, CborIdentityKey, CborIdentityRecord, ConeIdentity, DefinitionOriginSubject,
    NominalDeclarationOwner, PersistentId, SourceContextKey, ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, WirePath, encoded_length};

use super::OdrFreeHirFoundation;
use crate::{ExportDefinitionSourceV1, PersistentLexicalRootV1, SourceNominalId};

mod errors;
pub use errors::DefaultTemplateRootOriginValidationError;
type Error = DefaultTemplateRootOriginValidationError;

impl OdrFreeHirFoundation {
    /// Checks artifact membership, canonical identity and the definition's
    /// source/context. Parameter and inherited-provider contracts are separate.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_default_template_root_origin(
        &self,
        provider: ConeIdentity,
        identities: &ValidatedIdentityGraph,
        root: PersistentLexicalRootV1,
        origin: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Error> {
        meter.check_semantic_depth(1, path)?;
        meter.charge_nodes(1, path)?;
        let canonical = self.as_canonical();
        let (subject, expected_context, root_provider) = match root {
            PersistentLexicalRootV1::Function(id) => (
                DefinitionOriginSubject::Function(id),
                RootContext::Callable(CallableOwner::Function(id)),
                key(&canonical.functions, id, identities, root, meter, path)?.origin(),
            ),
            PersistentLexicalRootV1::GenericFunction(id) => (
                DefinitionOriginSubject::GenericFunction(id),
                RootContext::Callable(CallableOwner::GenericTemplate(id)),
                key(
                    &canonical.generic_functions,
                    id,
                    identities,
                    root,
                    meter,
                    path,
                )?
                .origin(),
            ),
            PersistentLexicalRootV1::Constructor(id) => (
                DefinitionOriginSubject::Constructor(id),
                RootContext::Callable(CallableOwner::Constructor(id)),
                key(&canonical.constructors, id, identities, root, meter, path)?.origin(),
            ),
            PersistentLexicalRootV1::EnumVariantConstructor(id) => {
                let variant = key(&canonical.enum_variants, id, identities, root, meter, path)?;
                let owner = variant.source_owner().ok_or(Error::RootProvider(root))?;
                let (context, declaration) = match owner {
                    SourceNominalId::Concrete(id) => (
                        NominalDeclarationOwner::Concrete(id),
                        key(&canonical.types, id, identities, root, meter, path)?,
                    ),
                    SourceNominalId::GenericTemplate(id) => (
                        NominalDeclarationOwner::GenericTemplate(id),
                        key(&canonical.generic_types, id, identities, root, meter, path)?,
                    ),
                };
                (
                    DefinitionOriginSubject::EnumVariant(id),
                    RootContext::Nominal(context),
                    declaration.origin(),
                )
            }
        };
        let body = origin.origin();
        if root_provider != provider || body.source().cone() != provider {
            return Err(Error::RootProvider(root));
        }
        meter.charge_work(
            u64::from(canonical.counts().definition_origins.max(1).ilog2()) + 1,
            path,
        )?;
        let declaration = self
            .definition_origin(subject)
            .ok_or(Error::RootOrigin(root))?
            .origin();
        meter.charge_work(
            u64::from(canonical.counts().source_contexts.max(1).ilog2()) + 1,
            path,
        )?;
        let context = self
            .source_context_key(body.context())
            .ok_or(Error::RootOrigin(root))?;
        let bytes = [declaration.source(), body.source(), context.source()]
            .into_iter()
            .fold(0_u64, |bytes, source| {
                bytes.saturating_add(source.logical_path().as_str().len() as u64)
            });
        meter.check_semantic_leaf(bytes, path)?;
        meter.charge_work(bytes.saturating_add(96), path)?;
        if declaration.source() != body.source()
            || context.source() != body.source()
            || !expected_context.matches(context)
        {
            return Err(Error::RootOrigin(root));
        }
        Ok(())
    }
}

fn key<'k, I, K>(
    records: &'k [CborIdentityRecord<I, K>],
    id: I,
    identities: &ValidatedIdentityGraph,
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
    let bytes = encoded_length(record.key()).map_err(|error| Error::Identity(error.to_string()))?;
    meter.check_semantic_leaf(bytes, path)?;
    meter.charge_work(bytes, path)?;
    let checked = identities
        .canonical_key::<I, K>(id)
        .map_err(|error| Error::Identity(error.to_string()))?;
    if checked.as_ref() != record.key() {
        return Err(Error::CanonicalKeyMismatch(root));
    }
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
