//! Import protocol references from shared source metadata.

use std::fmt;

use super::*;

pub(super) fn import_product<const N: usize>(
    foundation: &ImportedHirFoundation,
    entries: &[CoreProtocolEntryV1; N],
) -> Result<ImportedCoreProtocolProduct<N>, CoreProtocolImportError> {
    let entries = entries
        .iter()
        .map(|entry| import_entry(foundation, entry))
        .collect::<Result<Vec<_>, _>>()?;
    let Ok(entries) = entries.try_into() else {
        unreachable!("a fixed protocol product retains its array length")
    };
    Ok(ImportedCoreProtocolProduct { entries })
}

fn import_entry(
    foundation: &ImportedHirFoundation,
    entry: &CoreProtocolEntryV1,
) -> Result<ImportedCoreProtocolEntry, CoreProtocolImportError> {
    match entry {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(id)) => foundation
            .source_nominal(*id)
            .map(ImportedCoreProtocolNominal::Type)
            .map(ImportedCoreProtocolEntry::Nominal)
            .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, *id)),
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => foundation
            .generic_nominal(*id)
            .map(ImportedCoreProtocolNominal::GenericType)
            .map(ImportedCoreProtocolEntry::Nominal)
            .ok_or_else(|| missing(CoreProtocolIdentityKind::GenericType, *id)),
        CoreProtocolEntryV1::Callable(callable) => {
            import_callable(foundation, callable).map(ImportedCoreProtocolEntry::Callable)
        }
        CoreProtocolEntryV1::EnumVariant(id) => foundation
            .identity(*id)
            .map(ImportedCoreProtocolEntry::EnumVariant)
            .ok_or_else(|| missing(CoreProtocolIdentityKind::EnumVariant, *id)),
        CoreProtocolEntryV1::EnumVariantField(id) => foundation
            .identity(*id)
            .map(ImportedCoreProtocolEntry::EnumVariantField)
            .ok_or_else(|| missing(CoreProtocolIdentityKind::EnumVariantField, *id)),
        CoreProtocolEntryV1::DispatchSlot(id) => foundation
            .identity(*id)
            .map(ImportedCoreProtocolEntry::DispatchSlot)
            .ok_or_else(|| missing(CoreProtocolIdentityKind::DispatchSlot, *id)),
        CoreProtocolEntryV1::ExactType(id) => foundation
            .identity(*id)
            .map(ImportedCoreProtocolEntry::ExactType)
            .ok_or_else(|| missing(CoreProtocolIdentityKind::ExactType, *id)),
    }
}

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
