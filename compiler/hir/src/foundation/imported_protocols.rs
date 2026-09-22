//! Imported compiler protocol identities and native-boundary definitions.

use std::fmt;

mod native_boundary;
pub use native_boundary::{CoreNativeBoundaryImportError, ImportedCoreNativeBoundaryTypes};

use scoop_identity::{
    PersistentConstructorId, PersistentDispatchSlotId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExactTypeId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentId, PersistentTypeId, SignatureCallableShape, SourceDeclarationKind,
};

use super::{ImportedHirFoundation, ImportedHirId};
use crate::{
    COROUTINE_PROTOCOL_COUNT, CoreHirInterfaceV1, CoreProtocolCallableDefinitionV1,
    CoreProtocolCallableV1, CoreProtocolEntryV1, CoreProtocolNominalV1, EXCEPTION_PROTOCOL_COUNT,
    FFI_PROTOCOL_COUNT, FOREIGN_CALLBACK_PROTOCOL_COUNT, FUNDAMENTAL_TYPE_COUNT,
    ITERATION_PROTOCOL_COUNT, IntegerKind, NativeBoundaryCLayoutPolicy,
    NativeBoundaryDefinitionError, NativeBoundaryNominalOwner, NativeBoundaryNominalShape,
    NativeBoundaryTypeDefinitionRecord, OPTION_PROTOCOL_COUNT, SOURCE_LOCATION_PROTOCOL_COUNT,
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

impl ImportedCoreFundamentalTypeProtocol {
    pub fn unit(&self) -> ImportedHirId<PersistentTypeId> {
        concrete_nominal(&self.0, 0)
    }

    pub fn integer(&self, kind: IntegerKind) -> ImportedHirId<PersistentTypeId> {
        let index = IntegerKind::ALL
            .iter()
            .position(|candidate| *candidate == kind)
            .expect("the closed integer kind belongs to IntegerKind::ALL");
        concrete_nominal(&self.0, index + 1)
    }

    pub fn boolean(&self) -> ImportedHirId<PersistentTypeId> {
        concrete_nominal(&self.0, 9)
    }

    pub fn string(&self) -> ImportedHirId<PersistentTypeId> {
        concrete_nominal(&self.0, 10)
    }

    pub fn array(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 11)
    }

    pub fn mutable_array(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 12)
    }

    pub fn ptr(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 13)
    }

    pub fn fun_ptr(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 14)
    }
}

impl ImportedCoreOptionProtocol {
    pub fn option(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 0)
    }

    pub fn some(&self) -> ImportedHirId<PersistentEnumVariantId> {
        enum_variant(&self.0, 1)
    }

    pub fn some_payload(&self) -> ImportedHirId<PersistentEnumVariantFieldId> {
        enum_variant_field(&self.0, 2)
    }

    pub fn none(&self) -> ImportedHirId<PersistentEnumVariantId> {
        enum_variant(&self.0, 3)
    }
}

impl ImportedCoreIterationProtocol {
    pub fn iterator(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 0)
    }

    pub fn next(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 1)
    }

    pub fn next_dispatch(&self) -> ImportedHirId<PersistentDispatchSlotId> {
        dispatch_slot(&self.0, 2)
    }
}

impl ImportedCoreExceptionProtocol {
    pub fn throwable(&self) -> ImportedHirId<PersistentTypeId> {
        concrete_nominal(&self.0, 0)
    }

    pub fn throwable_constructor(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 1)
    }

    pub fn unwrap_exception(&self) -> ImportedHirId<PersistentTypeId> {
        concrete_nominal(&self.0, 2)
    }

    pub fn unwrap_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 3)
    }

    pub fn class_cast_exception(&self) -> ImportedHirId<PersistentTypeId> {
        concrete_nominal(&self.0, 4)
    }

    pub fn class_cast_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 5)
    }

    pub fn arithmetic_exception(&self) -> ImportedHirId<PersistentTypeId> {
        concrete_nominal(&self.0, 6)
    }

    pub fn arithmetic_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 7)
    }

    pub fn index_out_of_bounds_exception(&self) -> ImportedHirId<PersistentTypeId> {
        concrete_nominal(&self.0, 8)
    }

    pub fn index_out_of_bounds_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 9)
    }

    pub fn illegal_state_exception(&self) -> ImportedHirId<PersistentTypeId> {
        concrete_nominal(&self.0, 10)
    }

    pub fn illegal_state_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 11)
    }

    pub fn initialization_cycle_thrower(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 12)
    }
}

impl ImportedCoreCoroutineProtocol {
    pub fn continuation(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 0)
    }

    pub fn continuation_resume(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 1)
    }

    pub fn continuation_resume_dispatch(&self) -> ImportedHirId<PersistentDispatchSlotId> {
        dispatch_slot(&self.0, 2)
    }

    pub fn continuation_resume_with_exception(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 3)
    }

    pub fn continuation_resume_with_exception_dispatch(
        &self,
    ) -> ImportedHirId<PersistentDispatchSlotId> {
        dispatch_slot(&self.0, 4)
    }

    pub fn suspend_task(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 5)
    }

    pub fn suspend_task_run(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 6)
    }

    pub fn suspend_task_run_dispatch(&self) -> ImportedHirId<PersistentDispatchSlotId> {
        dispatch_slot(&self.0, 7)
    }

    pub fn suspend_registration(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 8)
    }

    pub fn suspend_registration_register(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 9)
    }

    pub fn suspend_registration_register_dispatch(
        &self,
    ) -> ImportedHirId<PersistentDispatchSlotId> {
        dispatch_slot(&self.0, 10)
    }

    pub fn start_coroutine(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 11)
    }

    pub fn suspend_coroutine(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 12)
    }
}

impl ImportedCoreFfiProtocol {
    pub fn ptr(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 0)
    }

    pub fn fun_ptr(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 1)
    }

    pub fn pinned_ptr(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 2)
    }

    pub fn gc_handle(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 3)
    }

    pub fn ptr_to_ulong(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 4)
    }

    pub fn ptr_cast(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 5)
    }

    pub fn ptr_load(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 6)
    }

    pub fn ptr_load_offset(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 7)
    }

    pub fn ptr_store(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 8)
    }

    pub fn ptr_store_offset(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 9)
    }

    pub fn ptr_plus(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 10)
    }

    pub fn ptr_minus(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 11)
    }

    pub fn address_of(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 12)
    }

    pub fn size_of(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 13)
    }

    pub fn align_of(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 14)
    }

    pub fn gc_pin_raw(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 15)
    }

    pub fn gc_unpin_raw(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 16)
    }

    pub fn gc_get_handle_raw(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 17)
    }

    pub fn gc_release_handle_raw(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 18)
    }
}

impl ImportedCoreForeignCallbackProtocol {
    pub fn callback(&self) -> ImportedHirId<PersistentGenericTypeId> {
        generic_nominal(&self.0, 0)
    }

    pub fn mode(&self) -> ImportedHirId<PersistentTypeId> {
        concrete_nominal(&self.0, 1)
    }

    pub fn reusable(&self) -> ImportedHirId<PersistentEnumVariantId> {
        enum_variant(&self.0, 2)
    }

    pub fn one_shot(&self) -> ImportedHirId<PersistentEnumVariantId> {
        enum_variant(&self.0, 3)
    }

    pub fn state(&self) -> ImportedHirId<PersistentTypeId> {
        concrete_nominal(&self.0, 4)
    }

    pub fn registered(&self) -> ImportedHirId<PersistentEnumVariantId> {
        enum_variant(&self.0, 5)
    }

    pub fn active(&self) -> ImportedHirId<PersistentEnumVariantId> {
        enum_variant(&self.0, 6)
    }

    pub fn completed(&self) -> ImportedHirId<PersistentEnumVariantId> {
        enum_variant(&self.0, 7)
    }

    pub fn failed(&self) -> ImportedHirId<PersistentEnumVariantId> {
        enum_variant(&self.0, 8)
    }

    pub fn failure_result(&self) -> ImportedHirId<PersistentExactTypeId> {
        exact_type(&self.0, 9)
    }

    pub fn register(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 10)
    }

    pub fn retain(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 11)
    }

    pub fn release(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 12)
    }

    pub fn query_state(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 13)
    }

    pub fn failure(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 14)
    }
}

impl ImportedCoreSourceLocationProtocol {
    pub fn location(&self) -> ImportedHirId<PersistentTypeId> {
        concrete_nominal(&self.0, 0)
    }

    pub fn current(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 1)
    }
}

/// Complete imported compiler-protocol authority. Every persistent subject
/// has already been resolved into the exact HIR identity session owned by the
/// trusted artifact; no declaration is copied into the current Cone arenas.
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
    fn import(
        foundation: &ImportedHirFoundation,
        interface: &CoreHirInterfaceV1,
    ) -> Result<Self, CoreProtocolImportError> {
        if foundation.origin() != scoop_identity::ConeIdentity::CORE {
            return Err(CoreProtocolImportError::FoundationNotCore(
                foundation.origin(),
            ));
        }
        let protocols = interface.compiler_protocols();
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

/// Compiler protocol identities and native-boundary definitions. Ordinary
/// public name lookup uses the shared dependency semantic world.
pub struct ImportedCoreInputs {
    protocols: ImportedCoreProtocols,
    native_boundary_types: ImportedCoreNativeBoundaryTypes,
}

impl ImportedCoreInputs {
    pub const fn protocols(&self) -> &ImportedCoreProtocols {
        &self.protocols
    }

    pub const fn native_boundary_types(&self) -> &ImportedCoreNativeBoundaryTypes {
        &self.native_boundary_types
    }
}

impl ImportedHirFoundation {
    /// Imports compiler protocols into the shared semantic identity session.
    pub fn import_core_inputs(
        &self,
        interface: &CoreHirInterfaceV1,
    ) -> Result<ImportedCoreInputs, CoreInterfaceImportError> {
        let protocols = ImportedCoreProtocols::import(self, interface)
            .map_err(CoreInterfaceImportError::Protocols)?;
        let native_boundary_types =
            ImportedCoreNativeBoundaryTypes::import(self, protocols.fundamental_types())
                .map_err(CoreInterfaceImportError::NativeBoundary)?;
        Ok(ImportedCoreInputs {
            protocols,
            native_boundary_types,
        })
    }
}

fn concrete_nominal<const N: usize>(
    product: &ImportedCoreProtocolProduct<N>,
    index: usize,
) -> ImportedHirId<PersistentTypeId> {
    match product.entries[index] {
        ImportedCoreProtocolEntry::Nominal(ImportedCoreProtocolNominal::Type(id)) => id,
        _ => unreachable!("an imported fixed protocol role retains its validated subject kind"),
    }
}

fn generic_nominal<const N: usize>(
    product: &ImportedCoreProtocolProduct<N>,
    index: usize,
) -> ImportedHirId<PersistentGenericTypeId> {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreInterfaceImportError {
    Protocols(CoreProtocolImportError),
    NativeBoundary(CoreNativeBoundaryImportError),
}

impl fmt::Display for CoreInterfaceImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocols(error) => error.fmt(formatter),
            Self::NativeBoundary(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreInterfaceImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Protocols(error) => Some(error),
            Self::NativeBoundary(error) => Some(error),
        }
    }
}
