//! Atomic imported view of the trusted core compiler protocols and prelude.

use std::fmt;

use scoop_identity::{
    PersistentConstructorId, PersistentDispatchSlotId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExactTypeId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentId, PersistentTypeId, SignatureCallableShape,
};

use super::{
    CorePreludeImportError, CorePreludeOnly, ImportedHirFoundation, ImportedHirId, ImportedHirSet,
};
use crate::{
    COROUTINE_PROTOCOL_COUNT, CoreHirInterfaceV1, CoreProtocolCallableDefinitionV1,
    CoreProtocolCallableV1, CoreProtocolEntryV1, CoreProtocolNominalV1, EXCEPTION_PROTOCOL_COUNT,
    FFI_PROTOCOL_COUNT, FOREIGN_CALLBACK_PROTOCOL_COUNT, FUNDAMENTAL_TYPE_COUNT,
    ITERATION_PROTOCOL_COUNT, IntrinsicFunctionKind, OPTION_PROTOCOL_COUNT,
    SOURCE_LOCATION_PROTOCOL_COUNT,
};

/// Imported nominal identity in the HIR semantic session that owns the
/// trusted core artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedCoreProtocolNominal {
    Type(ImportedHirId<PersistentTypeId>),
    GenericType(ImportedHirId<PersistentGenericTypeId>),
}

/// Imported callable declaration identity. The four source/generated domains
/// remain distinct after import and cannot be substituted for one another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedCoreProtocolCallableDefinition {
    Function(ImportedHirId<PersistentFunctionId>),
    GenericFunction(ImportedHirId<PersistentGenericFunctionId>),
    Constructor(ImportedHirId<PersistentConstructorId>),
    GeneratedCallable(ImportedHirId<PersistentGeneratedCallableId>),
}

/// One compiler protocol callable imported from the same checked foundation
/// as its complete source signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedCoreProtocolCallable {
    definition: ImportedCoreProtocolCallableDefinition,
    signature: SignatureCallableShape,
}

impl ImportedCoreProtocolCallable {
    pub const fn definition(&self) -> ImportedCoreProtocolCallableDefinition {
        self.definition
    }

    pub const fn signature(&self) -> &SignatureCallableShape {
        &self.signature
    }
}

/// Closed imported subject sum used by all fixed-role compiler protocols.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportedCoreProtocolEntry {
    Nominal(ImportedCoreProtocolNominal),
    Callable(ImportedCoreProtocolCallable),
    EnumVariant(ImportedHirId<PersistentEnumVariantId>),
    EnumVariantField(ImportedHirId<PersistentEnumVariantFieldId>),
    DispatchSlot(ImportedHirId<PersistentDispatchSlotId>),
    ExactType(ImportedHirId<PersistentExactTypeId>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImportedCoreProtocolProduct<const N: usize> {
    entries: [ImportedCoreProtocolEntry; N],
}

/// One total compiler-operation role mapped to its imported callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedCoreCompilerOperation {
    kind: IntrinsicFunctionKind,
    callable: ImportedCoreProtocolCallable,
}

impl ImportedCoreCompilerOperation {
    pub const fn kind(&self) -> IntrinsicFunctionKind {
        self.kind
    }

    pub const fn callable(&self) -> &ImportedCoreProtocolCallable {
        &self.callable
    }
}

/// Complete imported compiler-protocol authority. Every persistent subject
/// has already been resolved into the exact HIR identity session owned by the
/// trusted artifact; no declaration is copied into the current Cone arenas.
pub struct ImportedCoreProtocols<'a> {
    foundation: &'a ImportedHirFoundation,
    interface: &'a CoreHirInterfaceV1,
    fundamental_types: ImportedCoreProtocolProduct<FUNDAMENTAL_TYPE_COUNT>,
    option_protocol: ImportedCoreProtocolProduct<OPTION_PROTOCOL_COUNT>,
    iteration_protocol: ImportedCoreProtocolProduct<ITERATION_PROTOCOL_COUNT>,
    exception_protocol: ImportedCoreProtocolProduct<EXCEPTION_PROTOCOL_COUNT>,
    coroutine_protocol: ImportedCoreProtocolProduct<COROUTINE_PROTOCOL_COUNT>,
    ffi_protocol: ImportedCoreProtocolProduct<FFI_PROTOCOL_COUNT>,
    foreign_callback_protocol: ImportedCoreProtocolProduct<FOREIGN_CALLBACK_PROTOCOL_COUNT>,
    source_location_protocol: ImportedCoreProtocolProduct<SOURCE_LOCATION_PROTOCOL_COUNT>,
    compiler_operations: Vec<ImportedCoreCompilerOperation>,
}

impl<'a> ImportedCoreProtocols<'a> {
    fn import(
        foundation: &'a ImportedHirFoundation,
        interface: &'a CoreHirInterfaceV1,
    ) -> Result<Self, CoreProtocolImportError> {
        if foundation.origin() != scoop_identity::ConeIdentity::CORE {
            return Err(CoreProtocolImportError::FoundationNotCore(
                foundation.origin(),
            ));
        }
        let protocols = interface.compiler_protocols();
        Ok(Self {
            foundation,
            interface,
            fundamental_types: import_product(foundation, protocols.fundamental_types().entries())?,
            option_protocol: import_product(foundation, protocols.option_protocol().entries())?,
            iteration_protocol: import_product(
                foundation,
                protocols.iteration_protocol().entries(),
            )?,
            exception_protocol: import_product(
                foundation,
                protocols.exception_protocol().entries(),
            )?,
            coroutine_protocol: import_product(
                foundation,
                protocols.coroutine_protocol().entries(),
            )?,
            ffi_protocol: import_product(foundation, protocols.ffi_protocol().entries())?,
            foreign_callback_protocol: import_product(
                foundation,
                protocols.foreign_callback_protocol().entries(),
            )?,
            source_location_protocol: import_product(
                foundation,
                protocols.source_location_protocol().entries(),
            )?,
            compiler_operations: protocols
                .compiler_operation_protocol()
                .operations()
                .iter()
                .map(|operation| {
                    Ok(ImportedCoreCompilerOperation {
                        kind: operation.kind(),
                        callable: import_callable(foundation, operation.callable())?,
                    })
                })
                .collect::<Result<Vec<_>, CoreProtocolImportError>>()?,
        })
    }

    pub fn compiler_operations(&self) -> &[ImportedCoreCompilerOperation] {
        &self.compiler_operations
    }

    pub fn fixed_subject_count(&self) -> usize {
        self.fundamental_types.entries.len()
            + self.option_protocol.entries.len()
            + self.iteration_protocol.entries.len()
            + self.exception_protocol.entries.len()
            + self.coroutine_protocol.entries.len()
            + self.ffi_protocol.entries.len()
            + self.foreign_callback_protocol.entries.len()
            + self.source_location_protocol.entries.len()
    }

    #[doc(hidden)]
    pub fn belongs_to(
        &self,
        foundation: &ImportedHirFoundation,
        interface: &CoreHirInterfaceV1,
    ) -> bool {
        std::ptr::eq(self.foundation, foundation) && std::ptr::eq(self.interface, interface)
    }
}

/// The only ordinary-Cone HIR import granted by a trusted core artifact.
/// Prelude lookup and compiler protocols are constructed as one value and
/// cannot be paired independently with another artifact.
pub struct ImportedCoreInputs<'a> {
    prelude: ImportedHirSet<'a, CorePreludeOnly>,
    protocols: ImportedCoreProtocols<'a>,
}

impl<'a> ImportedCoreInputs<'a> {
    pub const fn prelude(&self) -> &ImportedHirSet<'a, CorePreludeOnly> {
        &self.prelude
    }

    pub const fn protocols(&self) -> &ImportedCoreProtocols<'a> {
        &self.protocols
    }
}

impl ImportedHirFoundation {
    /// Atomically imports the prelude and every compiler protocol from one
    /// validated core interface into this exact semantic identity session.
    pub fn import_core_inputs<'a>(
        &'a self,
        interface: &'a CoreHirInterfaceV1,
        strong_callable_bindings: &'a [scoop_identity::PersistentExportBindingId],
    ) -> Result<ImportedCoreInputs<'a>, CoreInterfaceImportError> {
        let protocols = ImportedCoreProtocols::import(self, interface)
            .map_err(CoreInterfaceImportError::Protocols)?;
        let prelude = self
            .import_core_prelude(interface, strong_callable_bindings)
            .map_err(CoreInterfaceImportError::Prelude)?;
        Ok(ImportedCoreInputs { prelude, protocols })
    }
}

fn import_product<const N: usize>(
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
            .identity(*id)
            .map(ImportedCoreProtocolNominal::Type)
            .map(ImportedCoreProtocolEntry::Nominal)
            .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, *id)),
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => foundation
            .identity(*id)
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

fn import_callable(
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
    Ok(ImportedCoreProtocolCallable {
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
    FoundationNotCore(scoop_identity::ConeIdentity),
    MissingIdentity {
        kind: CoreProtocolIdentityKind,
        identity: [u8; 32],
    },
}

impl fmt::Display for CoreProtocolImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot import trusted core protocols: {self:?}")
    }
}

impl std::error::Error for CoreProtocolImportError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreInterfaceImportError {
    Prelude(CorePreludeImportError),
    Protocols(CoreProtocolImportError),
}

impl fmt::Display for CoreInterfaceImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prelude(error) => error.fmt(formatter),
            Self::Protocols(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreInterfaceImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Prelude(error) => Some(error),
            Self::Protocols(error) => Some(error),
        }
    }
}
