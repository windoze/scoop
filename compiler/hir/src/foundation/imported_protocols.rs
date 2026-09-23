//! Imported compiler protocol identities.

mod import;
mod roles;

use import::import_product;
pub use import::{CoreProtocolIdentityKind, CoreProtocolImportError};

use scoop_identity::{
    ConeIdentity, DefinitionOriginSubject, PersistentConstructorId, PersistentDispatchSlotId,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentExactTypeId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentId, PersistentTypeId, SignatureCallableShape,
};

use super::{ImportedHirFoundation, ImportedHirId, ImportedHirNominal};
use crate::{
    COROUTINE_PROTOCOL_COUNT, CompilerProtocolDefinitionsV1, CoreProtocolCallableDefinitionV1,
    CoreProtocolCallableV1, CoreProtocolEntryV1, CoreProtocolNominalV1, EXCEPTION_PROTOCOL_COUNT,
    FFI_PROTOCOL_COUNT, FOREIGN_CALLBACK_PROTOCOL_COUNT, FUNDAMENTAL_TYPE_COUNT,
    ITERATION_PROTOCOL_COUNT, IntegerKind, OPTION_PROTOCOL_COUNT, SOURCE_LOCATION_PROTOCOL_COUNT,
};

/// Imported nominal identity in the HIR semantic session that owns the
/// dependency artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedCoreProtocolNominal {
    Type(ImportedHirNominal<PersistentTypeId>),
    GenericType(ImportedHirNominal<PersistentGenericTypeId>),
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
    provider: ConeIdentity,
    definition: ImportedCoreProtocolCallableDefinition,
    signature: SignatureCallableShape,
}

impl ImportedCoreProtocolCallable {
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

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

macro_rules! imported_protocol_product {
    ($name:ident, $count:ident) => {
        #[derive(Clone, Debug)]
        pub struct $name(ImportedCoreProtocolProduct<$count>);

        impl $name {
            const fn subject_count(&self) -> usize {
                self.0.entries.len()
            }
        }
    };
}

imported_protocol_product!(ImportedCoreFundamentalTypeProtocol, FUNDAMENTAL_TYPE_COUNT);
imported_protocol_product!(ImportedCoreOptionProtocol, OPTION_PROTOCOL_COUNT);
imported_protocol_product!(ImportedCoreIterationProtocol, ITERATION_PROTOCOL_COUNT);
imported_protocol_product!(ImportedCoreExceptionProtocol, EXCEPTION_PROTOCOL_COUNT);
imported_protocol_product!(ImportedCoreCoroutineProtocol, COROUTINE_PROTOCOL_COUNT);
imported_protocol_product!(ImportedCoreFfiProtocol, FFI_PROTOCOL_COUNT);
imported_protocol_product!(
    ImportedCoreForeignCallbackProtocol,
    FOREIGN_CALLBACK_PROTOCOL_COUNT
);
imported_protocol_product!(
    ImportedCoreSourceLocationProtocol,
    SOURCE_LOCATION_PROTOCOL_COUNT
);

/// Complete imported compiler-protocol references. Every persistent subject
/// has already been resolved into the exact HIR identity session owned by the
/// dependency artifact; no declaration is copied into the current Cone arenas.
#[derive(Clone, Debug)]
pub struct ImportedCoreProtocols {
    fundamental_types: ImportedCoreFundamentalTypeProtocol,
    option_protocol: ImportedCoreOptionProtocol,
    iteration_protocol: ImportedCoreIterationProtocol,
    exception_protocol: ImportedCoreExceptionProtocol,
    coroutine_protocol: ImportedCoreCoroutineProtocol,
    ffi_protocol: ImportedCoreFfiProtocol,
    foreign_callback_protocol: ImportedCoreForeignCallbackProtocol,
    source_location_protocol: ImportedCoreSourceLocationProtocol,
}

impl ImportedCoreProtocols {
    pub(crate) fn import(
        foundation: &ImportedHirFoundation,
        protocols: &crate::CoreCompilerProtocolSurfaceV1,
    ) -> Result<Self, CoreProtocolImportError> {
        Ok(Self {
            fundamental_types: ImportedCoreFundamentalTypeProtocol(import_product(
                foundation,
                protocols.fundamental_types().entries(),
            )?),
            option_protocol: ImportedCoreOptionProtocol(import_product(
                foundation,
                protocols.option_protocol().entries(),
            )?),
            iteration_protocol: ImportedCoreIterationProtocol(import_product(
                foundation,
                protocols.iteration_protocol().entries(),
            )?),
            exception_protocol: ImportedCoreExceptionProtocol(import_product(
                foundation,
                protocols.exception_protocol().entries(),
            )?),
            coroutine_protocol: ImportedCoreCoroutineProtocol(import_product(
                foundation,
                protocols.coroutine_protocol().entries(),
            )?),
            ffi_protocol: ImportedCoreFfiProtocol(import_product(
                foundation,
                protocols.ffi_protocol().entries(),
            )?),
            foreign_callback_protocol: ImportedCoreForeignCallbackProtocol(import_product(
                foundation,
                protocols.foreign_callback_protocol().entries(),
            )?),
            source_location_protocol: ImportedCoreSourceLocationProtocol(import_product(
                foundation,
                protocols.source_location_protocol().entries(),
            )?),
        })
    }

    pub const fn fundamental_types(&self) -> &ImportedCoreFundamentalTypeProtocol {
        &self.fundamental_types
    }

    pub const fn option(&self) -> &ImportedCoreOptionProtocol {
        &self.option_protocol
    }

    pub const fn iteration(&self) -> &ImportedCoreIterationProtocol {
        &self.iteration_protocol
    }

    pub const fn exceptions(&self) -> &ImportedCoreExceptionProtocol {
        &self.exception_protocol
    }

    pub const fn coroutines(&self) -> &ImportedCoreCoroutineProtocol {
        &self.coroutine_protocol
    }

    pub const fn ffi(&self) -> &ImportedCoreFfiProtocol {
        &self.ffi_protocol
    }

    pub const fn foreign_callbacks(&self) -> &ImportedCoreForeignCallbackProtocol {
        &self.foreign_callback_protocol
    }

    pub const fn source_location(&self) -> &ImportedCoreSourceLocationProtocol {
        &self.source_location_protocol
    }

    pub fn fixed_subject_count(&self) -> usize {
        self.fundamental_types.subject_count()
            + self.option_protocol.subject_count()
            + self.iteration_protocol.subject_count()
            + self.exception_protocol.subject_count()
            + self.coroutine_protocol.subject_count()
            + self.ffi_protocol.subject_count()
            + self.foreign_callback_protocol.subject_count()
            + self.source_location_protocol.subject_count()
    }
}

/// Compiler protocol identities. Ordinary
/// public name lookup uses the shared dependency semantic world.
#[derive(Clone)]
pub struct ImportedCoreInputs {
    protocols: ImportedCoreProtocols,
}

impl ImportedCoreInputs {
    pub const fn protocols(&self) -> &ImportedCoreProtocols {
        &self.protocols
    }
}

impl ImportedHirFoundation {
    /// Imports compiler protocols into the shared semantic identity session.
    pub fn import_core_inputs(
        &self,
        interface: &CompilerProtocolDefinitionsV1,
    ) -> Result<ImportedCoreInputs, CoreProtocolImportError> {
        let protocols = ImportedCoreProtocols::import(self, interface.compiler_protocols())?;
        Ok(ImportedCoreInputs { protocols })
    }
}

fn concrete_nominal<const N: usize>(
    product: &ImportedCoreProtocolProduct<N>,
    index: usize,
) -> ImportedHirNominal<PersistentTypeId> {
    match product.entries[index] {
        ImportedCoreProtocolEntry::Nominal(ImportedCoreProtocolNominal::Type(id)) => id,
        _ => unreachable!("an imported fixed protocol role retains its validated subject kind"),
    }
}

fn generic_nominal<const N: usize>(
    product: &ImportedCoreProtocolProduct<N>,
    index: usize,
) -> ImportedHirNominal<PersistentGenericTypeId> {
    match product.entries[index] {
        ImportedCoreProtocolEntry::Nominal(ImportedCoreProtocolNominal::GenericType(id)) => id,
        _ => unreachable!("an imported fixed protocol role retains its validated subject kind"),
    }
}

fn callable<const N: usize>(
    product: &ImportedCoreProtocolProduct<N>,
    index: usize,
) -> &ImportedCoreProtocolCallable {
    match &product.entries[index] {
        ImportedCoreProtocolEntry::Callable(callable) => callable,
        _ => unreachable!("an imported fixed protocol role retains its validated subject kind"),
    }
}

fn enum_variant<const N: usize>(
    product: &ImportedCoreProtocolProduct<N>,
    index: usize,
) -> ImportedHirId<PersistentEnumVariantId> {
    match product.entries[index] {
        ImportedCoreProtocolEntry::EnumVariant(id) => id,
        _ => unreachable!("an imported fixed protocol role retains its validated subject kind"),
    }
}

fn enum_variant_field<const N: usize>(
    product: &ImportedCoreProtocolProduct<N>,
    index: usize,
) -> ImportedHirId<PersistentEnumVariantFieldId> {
    match product.entries[index] {
        ImportedCoreProtocolEntry::EnumVariantField(id) => id,
        _ => unreachable!("an imported fixed protocol role retains its validated subject kind"),
    }
}

fn dispatch_slot<const N: usize>(
    product: &ImportedCoreProtocolProduct<N>,
    index: usize,
) -> ImportedHirId<PersistentDispatchSlotId> {
    match product.entries[index] {
        ImportedCoreProtocolEntry::DispatchSlot(id) => id,
        _ => unreachable!("an imported fixed protocol role retains its validated subject kind"),
    }
}

fn exact_type<const N: usize>(
    product: &ImportedCoreProtocolProduct<N>,
    index: usize,
) -> ImportedHirId<PersistentExactTypeId> {
    match product.entries[index] {
        ImportedCoreProtocolEntry::ExactType(id) => id,
        _ => unreachable!("an imported fixed protocol role retains its validated subject kind"),
    }
}

#[cfg(test)]
mod tests;
