//! Binds nested descriptor roles and paths to actual artifact identity records.
use super::*;
use crate::cross_cone_type_semantics::source_authority::binding_keys;
use DefaultNestedCallableIdentityV1 as Identity;
use DefaultSourceNestedIdentityFailureV1 as Failure;
use scoop_identity::{
    CborIdentityKey, CborIdentityRecord, DeclarationScope, GeneratedCallableKey,
    LexicalCallableRole, PersistentId, SourceDeclarationKey, StructuralDefinitionPath,
};

pub(super) fn validate<'p, 's, 'a, 'f>(
    current: &BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>,
    dependencies: &[&BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>],
    nested: &DefaultSourceNestedCallablesV1<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error> {
    for occurrence in nested.occurrences() {
        let descriptor = occurrence.descriptor();
        let identity = descriptor.identity();
        let definition_path = descriptor.definition_path();
        let origin = occurrence.definition_origin();
        let source = sources::provider(
            current,
            dependencies,
            origin.origin().source().cone(),
            meter,
            path,
        )?;
        let foundation = source.members().nominals.foundation;
        let canonical = foundation.foundation.as_canonical();
        let key_path = match identity {
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
                local_path(key, identity, origin, meter, path)?
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
                            ..
                        },
                    )
                    | (
                        Identity::AnonymousFunction(_),
                        GeneratedCallableKey::Lexical {
                            role: LexicalCallableRole::AnonymousFunctionBody,
                            path,
                            ..
                        },
                    )
                    | (
                        Identity::CallableReference(_),
                        GeneratedCallableKey::CallableReferenceInvoke { path, .. },
                    ) => path,
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
    }
    Ok(())
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
    foundation: &BoundTypeFoundationSourcesV1<'_>,
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
        .map_err(|e| Error::Nominal(Box::new(NominalSourceBindingError::Foundation(e))))?;
    Ok(record.key())
}
fn failure(identity: Identity, reason: Failure) -> Error {
    Error::NestedIdentity { identity, reason }
}
