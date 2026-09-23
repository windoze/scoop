//! Validates a nested descriptor against its actual artifact identity records.
use super::*;
use DefaultNestedCallableIdentityV1 as Identity;
use DefaultSourceNestedIdentityFailureV1 as Failure;
use scoop_identity::{
    CborIdentityKey, CborIdentityRecord, DeclarationScope, GeneratedCallableKey,
    LexicalCallableRole, PersistentId, SourceDeclarationKey, StructuralDefinitionPath,
};

mod binders;
mod errors;
mod parent;
pub use errors::{DefaultNestedIdentityValidationError, DefaultSourceNestedIdentityFailureV1};
type Error = DefaultNestedIdentityValidationError;

impl DefaultTargetIdentityQueriesV1<'_> {
    /// Validates the borrowed occurrence's identity, source, parent and full
    /// lexical binder count. Its capture ABI and call access remain separate.
    pub fn validate_nested_callable_identity(
        &self,
        descriptor: DefaultSourceNestedCallableDescriptorV1<'_>,
        origin: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Error> {
        let foundation = self;
        let identity = descriptor.identity();
        if origin.origin().source().cone() != self.provider {
            return Err(failure(identity, Failure::DefinitionSource));
        }
        let definition_path = descriptor.definition_path();
        let canonical = foundation.foundation.as_canonical();
        let (key_path, parent) = match identity {
            Identity::LocalFunction(declaration) => {
                let key = match declaration {
                    CallableTemplateOrigin::Function(id) => key(
                        foundation,
                        canonical.type_source_function_records(),
                        id,
                        identity,
                        meter,
                        path,
                    )?,
                    CallableTemplateOrigin::GenericFunction(id) => key(
                        foundation,
                        canonical.type_source_generic_function_records(),
                        id,
                        identity,
                        meter,
                        path,
                    )?,
                    _ => return Err(failure(identity, Failure::Kind)),
                };
                (
                    local_path(key, identity, origin, meter, path)?,
                    parent::local(key, identity)?,
                )
            }
            Identity::Lambda(id)
            | Identity::AnonymousFunction(id)
            | Identity::CallableReference(id) => {
                let key = key(
                    foundation,
                    canonical.type_source_generated_callable_records(),
                    id,
                    identity,
                    meter,
                    path,
                )?;
                match (identity, key) {
                    (
                        Identity::Lambda(_),
                        GeneratedCallableKey::Lexical {
                            role: LexicalCallableRole::LambdaBody,
                            path,
                            parent,
                        },
                    )
                    | (
                        Identity::AnonymousFunction(_),
                        GeneratedCallableKey::Lexical {
                            role: LexicalCallableRole::AnonymousFunctionBody,
                            path,
                            parent,
                        },
                    )
                    | (
                        Identity::CallableReference(_),
                        GeneratedCallableKey::CallableReferenceInvoke { path, parent },
                    ) => (path, parent.template()),
                    _ => return Err(failure(identity, Failure::Kind)),
                }
            }
        };
        meter.charge_work(
            (key_path.segments().len() as u64)
                .saturating_add(definition_path.segments().len() as u64),
            path,
        )?;
        if key_path != definition_path {
            return Err(failure(identity, Failure::DefinitionPath));
        }
        parent::validate(foundation, parent, identity, origin, meter, path)?;
        binders::validate(foundation, parent, descriptor, meter, path)?;
        Ok(())
    }
}

fn local_path<'k>(
    key: &'k SourceDeclarationKey,
    identity: Identity,
    origin: &ExportDefinitionSourceV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<&'k StructuralDefinitionPath, Error> {
    let DeclarationScope::LexicalScoped {
        source,
        path: definition_path,
    } = key.scope()
    else {
        return Err(failure(identity, Failure::Kind));
    };
    meter.charge_work(
        (source.logical_path().as_str().len() as u64)
            .saturating_add(origin.origin().source().logical_path().as_str().len() as u64)
            .saturating_add(64),
        path,
    )?;
    if source != origin.origin().source() {
        return Err(failure(identity, Failure::DefinitionSource));
    }
    Ok(definition_path)
}

fn key<'k, I, K>(
    foundation: &DefaultTargetIdentityQueriesV1<'_>,
    records: &'k [CborIdentityRecord<I, K>],
    id: I,
    identity: Identity,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<&'k K, Error>
where
    I: PersistentId + 'static,
    K: CborIdentityKey<I> + Eq + Clone + Send + Sync + 'static,
{
    // Foundation declaration records preserve dependency order, not ID order.
    meter.check_table_entries(records.len() as u64, path)?;
    meter.charge_work((records.len() as u64).saturating_mul(64), path)?;
    let record = records
        .iter()
        .find(|r| r.id() == id)
        .ok_or_else(|| failure(identity, Failure::MissingArtifactRecord))?;
    binding_keys::verify(id, record.key(), foundation.identities, meter, path)
        .map_err(Error::Foundation)?;
    Ok(record.key())
}
fn failure(identity: Identity, reason: Failure) -> Error {
    Error::Identity { identity, reason }
}

fn query_cost(count: usize, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), Error> {
    meter.charge_work(u64::from(count.max(1).ilog2()) + 1, path)?;
    Ok(())
}
