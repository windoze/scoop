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
                    CallableTemplateOrigin::Function(id) => {
                        key(canonical.type_source_function_records(), id, identity)?
                    }
                    CallableTemplateOrigin::GenericFunction(id) => key(
                        canonical.type_source_generic_function_records(),
                        id,
                        identity,
                    )?,
                    _ => return Err(failure(identity, Failure::Kind)),
                };
                (
                    local_path(key, identity, origin)?,
                    parent::local(key, identity)?,
                )
            }
            Identity::Lambda(id)
            | Identity::AnonymousFunction(id)
            | Identity::CallableReference(id) => {
                let key = key(
                    canonical.type_source_generated_callable_records(),
                    id,
                    identity,
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

        if key_path != definition_path {
            return Err(failure(identity, Failure::DefinitionPath));
        }
        parent::validate(foundation, parent, identity, origin)?;
        binders::validate(foundation, parent, descriptor, path)?;
        Ok(())
    }
}

fn local_path<'k>(
    key: &'k SourceDeclarationKey,
    identity: Identity,
    origin: &ExportDefinitionSourceV1,
) -> Result<&'k StructuralDefinitionPath, Error> {
    let DeclarationScope::LexicalScoped {
        source,
        path: definition_path,
    } = key.scope()
    else {
        return Err(failure(identity, Failure::Kind));
    };

    if source != origin.origin().source() {
        return Err(failure(identity, Failure::DefinitionSource));
    }
    Ok(definition_path)
}

fn key<I, K>(records: &[CborIdentityRecord<I, K>], id: I, identity: Identity) -> Result<&K, Error>
where
    I: PersistentId + 'static,
    K: CborIdentityKey<I> + Eq + Clone + Send + Sync + 'static,
{
    // Foundation declaration records preserve dependency order, not ID order.

    let record = records
        .iter()
        .find(|r| r.id() == id)
        .ok_or_else(|| failure(identity, Failure::MissingArtifactRecord))?;
    Ok(record.key())
}
fn failure(identity: Identity, reason: Failure) -> Error {
    Error::Identity { identity, reason }
}
