//! Import protocol references from shared source metadata.

use std::fmt;

use super::*;

mod products;

pub(super) fn import_callable(
    foundation: &ImportedHirFoundation,
    callable: &CoreProtocolCallableV1,
) -> Result<ImportedCoreProtocolCallable, CoreProtocolImportError> {
    let definition = match callable.definition() {
        CoreProtocolCallableDefinitionV1::Function(id) => foundation
            .identity(id)
            .map(ImportedCoreProtocolCallableDefinition::Function)
            .ok_or_else(|| missing(CoreProtocolIdentityKind::Function, id))?,
        CoreProtocolCallableDefinitionV1::GenericFunction(id) => foundation
            .identity(id)
            .map(ImportedCoreProtocolCallableDefinition::GenericFunction)
            .ok_or_else(|| missing(CoreProtocolIdentityKind::GenericFunction, id))?,
        CoreProtocolCallableDefinitionV1::Constructor(id) => foundation
            .identity(id)
            .map(ImportedCoreProtocolCallableDefinition::Constructor)
            .ok_or_else(|| missing(CoreProtocolIdentityKind::Constructor, id))?,
        CoreProtocolCallableDefinitionV1::GeneratedCallable(id) => foundation
            .identity(id)
            .map(ImportedCoreProtocolCallableDefinition::GeneratedCallable)
            .ok_or_else(|| missing(CoreProtocolIdentityKind::GeneratedCallable, id))?,
    };
    let subject = callable.definition().origin_subject();
    let provider = foundation
        .canonical_for_semantic_authority()
        .definition_origin(subject)
        .ok_or(CoreProtocolImportError::MissingDefinitionOrigin { subject })?
        .origin()
        .source()
        .cone();
    Ok(ImportedCoreProtocolCallable {
        provider,
        definition,
        signature: callable.signature().clone(),
    })
}

fn missing<I: PersistentId>(kind: CoreProtocolIdentityKind, id: I) -> CoreProtocolImportError {
    CoreProtocolImportError::MissingIdentity {
        kind,
        identity: *id.as_array(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreProtocolIdentityKind {
    Type,
    GenericType,
    Function,
    GenericFunction,
    Constructor,
    GeneratedCallable,
    EnumVariant,
    EnumVariantField,
    DispatchSlot,
    ExactType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreProtocolImportError {
    MissingDefinitionOrigin {
        subject: DefinitionOriginSubject,
    },
    MissingIdentity {
        kind: CoreProtocolIdentityKind,
        identity: [u8; 32],
    },
}

impl fmt::Display for CoreProtocolImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot resolve compiler protocol identities: {self:?}"
        )
    }
}

impl std::error::Error for CoreProtocolImportError {}
