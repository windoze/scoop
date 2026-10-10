//! Imported compiler protocol identities.

mod import;
mod roles;

pub use import::{CoreProtocolIdentityKind, CoreProtocolImportError};

use scoop_identity::{
    ConeIdentity, DefinitionOriginSubject, PersistentConstructorId, PersistentDispatchSlotId,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentExactTypeId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentId, PersistentTypeId, SignatureCallableShape,
};

use super::{ImportedHirFoundation, ImportedHirId, ImportedHirNominal};
use crate::{
    COROUTINE_PROTOCOL_COUNT, CoreCompilerProtocolSurfaceV1, CoreProtocolCallableDefinitionV1,
    CoreProtocolCallableV1, EXCEPTION_PROTOCOL_COUNT, FFI_PROTOCOL_COUNT,
    FOREIGN_CALLBACK_PROTOCOL_COUNT, FUNDAMENTAL_TYPE_COUNT, ITERATION_PROTOCOL_COUNT, IntegerKind,
    OPTION_PROTOCOL_COUNT, SOURCE_LOCATION_PROTOCOL_COUNT,
};

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

#[derive(Clone, Debug)]
pub struct ImportedCoreFundamentalTypeProtocol {
    unit: ImportedHirNominal<PersistentTypeId>,
    int8: ImportedHirNominal<PersistentTypeId>,
    int16: ImportedHirNominal<PersistentTypeId>,
    int32: ImportedHirNominal<PersistentTypeId>,
    int64: ImportedHirNominal<PersistentTypeId>,
    uint8: ImportedHirNominal<PersistentTypeId>,
    uint16: ImportedHirNominal<PersistentTypeId>,
    uint32: ImportedHirNominal<PersistentTypeId>,
    uint64: ImportedHirNominal<PersistentTypeId>,
    boolean: ImportedHirNominal<PersistentTypeId>,
    string: ImportedHirNominal<PersistentTypeId>,
    array: ImportedHirNominal<PersistentGenericTypeId>,
    mutable_array: ImportedHirNominal<PersistentGenericTypeId>,
    ptr: ImportedHirNominal<PersistentGenericTypeId>,
    fun_ptr: ImportedHirNominal<PersistentGenericTypeId>,
    character: ImportedHirNominal<PersistentTypeId>,
    float: ImportedHirNominal<PersistentTypeId>,
    double: ImportedHirNominal<PersistentTypeId>,
    any: ImportedHirNominal<PersistentTypeId>,
    nothing: ImportedHirNominal<PersistentTypeId>,
}

#[derive(Clone, Debug)]
pub struct ImportedCoreOptionProtocol {
    option: ImportedHirNominal<PersistentGenericTypeId>,
    some: ImportedHirId<PersistentEnumVariantId>,
    some_payload: ImportedHirId<PersistentEnumVariantFieldId>,
    none: ImportedHirId<PersistentEnumVariantId>,
}

#[derive(Clone, Debug)]
pub struct ImportedCoreIterationProtocol {
    iterator: ImportedHirNominal<PersistentGenericTypeId>,
    next: ImportedCoreProtocolCallable,
    next_dispatch: ImportedHirId<PersistentDispatchSlotId>,
}

#[derive(Clone, Debug)]
pub struct ImportedCoreExceptionProtocol {
    throwable: ImportedHirNominal<PersistentTypeId>,
    throwable_constructor: ImportedCoreProtocolCallable,
    unwrap_exception: ImportedHirNominal<PersistentTypeId>,
    unwrap_exception_constructor: ImportedCoreProtocolCallable,
    class_cast_exception: ImportedHirNominal<PersistentTypeId>,
    class_cast_exception_constructor: ImportedCoreProtocolCallable,
    arithmetic_exception: ImportedHirNominal<PersistentTypeId>,
    arithmetic_exception_constructor: ImportedCoreProtocolCallable,
    index_out_of_bounds_exception: ImportedHirNominal<PersistentTypeId>,
    index_out_of_bounds_exception_constructor: ImportedCoreProtocolCallable,
    illegal_state_exception: ImportedHirNominal<PersistentTypeId>,
    illegal_state_exception_constructor: ImportedCoreProtocolCallable,
    initialization_cycle_thrower: ImportedCoreProtocolCallable,
    illegal_argument_exception: ImportedHirNominal<PersistentTypeId>,
    illegal_argument_exception_constructor: ImportedCoreProtocolCallable,
    missing_context_exception: ImportedHirNominal<PersistentTypeId>,
    missing_context_exception_constructor: ImportedCoreProtocolCallable,
}

#[derive(Clone, Debug)]
pub struct ImportedCoreCoroutineProtocol {
    continuation: ImportedHirNominal<PersistentGenericTypeId>,
    continuation_resume: ImportedCoreProtocolCallable,
    continuation_resume_dispatch: ImportedHirId<PersistentDispatchSlotId>,
    continuation_resume_with_exception: ImportedCoreProtocolCallable,
    continuation_resume_with_exception_dispatch: ImportedHirId<PersistentDispatchSlotId>,
    suspend_task: ImportedHirNominal<PersistentGenericTypeId>,
    suspend_task_run: ImportedCoreProtocolCallable,
    suspend_task_run_dispatch: ImportedHirId<PersistentDispatchSlotId>,
    suspend_registration: ImportedHirNominal<PersistentGenericTypeId>,
    suspend_registration_register: ImportedCoreProtocolCallable,
    suspend_registration_register_dispatch: ImportedHirId<PersistentDispatchSlotId>,
    start_coroutine: ImportedCoreProtocolCallable,
    suspend_coroutine: ImportedCoreProtocolCallable,
}

#[derive(Clone, Debug)]
pub struct ImportedCoreFfiProtocol {
    ptr: ImportedHirNominal<PersistentGenericTypeId>,
    fun_ptr: ImportedHirNominal<PersistentGenericTypeId>,
    pinned_ptr: ImportedHirNominal<PersistentGenericTypeId>,
    gc_handle: ImportedHirNominal<PersistentGenericTypeId>,
    ptr_to_ulong: ImportedCoreProtocolCallable,
    ptr_cast: ImportedCoreProtocolCallable,
    ptr_load: ImportedCoreProtocolCallable,
    ptr_load_offset: ImportedCoreProtocolCallable,
    ptr_store: ImportedCoreProtocolCallable,
    ptr_store_offset: ImportedCoreProtocolCallable,
    ptr_plus: ImportedCoreProtocolCallable,
    ptr_minus: ImportedCoreProtocolCallable,
    address_of: ImportedCoreProtocolCallable,
    size_of: ImportedCoreProtocolCallable,
    align_of: ImportedCoreProtocolCallable,
    gc_pin_raw: ImportedCoreProtocolCallable,
    gc_unpin_raw: ImportedCoreProtocolCallable,
    gc_get_handle_raw: ImportedCoreProtocolCallable,
    gc_release_handle_raw: ImportedCoreProtocolCallable,
}

#[derive(Clone, Debug)]
pub struct ImportedCoreForeignCallbackProtocol {
    callback: ImportedHirNominal<PersistentGenericTypeId>,
    mode: ImportedHirNominal<PersistentTypeId>,
    reusable: ImportedHirId<PersistentEnumVariantId>,
    one_shot: ImportedHirId<PersistentEnumVariantId>,
    state: ImportedHirNominal<PersistentTypeId>,
    registered: ImportedHirId<PersistentEnumVariantId>,
    active: ImportedHirId<PersistentEnumVariantId>,
    completed: ImportedHirId<PersistentEnumVariantId>,
    failed: ImportedHirId<PersistentEnumVariantId>,
    failure_result: ImportedHirId<PersistentExactTypeId>,
    register: ImportedCoreProtocolCallable,
    retain: ImportedCoreProtocolCallable,
    release: ImportedCoreProtocolCallable,
    query_state: ImportedCoreProtocolCallable,
    failure: ImportedCoreProtocolCallable,
}

#[derive(Clone, Debug)]
pub struct ImportedCoreSourceLocationProtocol {
    location: ImportedHirNominal<PersistentTypeId>,
    current: ImportedCoreProtocolCallable,
}

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
    program_arguments: ImportedCoreProtocolCallable,
}

impl ImportedCoreProtocols {
    pub const fn program_arguments(&self) -> &ImportedCoreProtocolCallable {
        &self.program_arguments
    }

    pub(crate) fn import(
        foundation: &ImportedHirFoundation,
        protocols: &crate::CoreCompilerProtocolSurfaceV1,
    ) -> Result<Self, CoreProtocolImportError> {
        Ok(Self {
            fundamental_types: ImportedCoreFundamentalTypeProtocol::import(
                foundation,
                protocols.fundamental_types(),
            )?,
            option_protocol: ImportedCoreOptionProtocol::import(
                foundation,
                protocols.option_protocol(),
            )?,
            iteration_protocol: ImportedCoreIterationProtocol::import(
                foundation,
                protocols.iteration_protocol(),
            )?,
            exception_protocol: ImportedCoreExceptionProtocol::import(
                foundation,
                protocols.exception_protocol(),
            )?,
            coroutine_protocol: ImportedCoreCoroutineProtocol::import(
                foundation,
                protocols.coroutine_protocol(),
            )?,
            ffi_protocol: ImportedCoreFfiProtocol::import(foundation, protocols.ffi_protocol())?,
            foreign_callback_protocol: ImportedCoreForeignCallbackProtocol::import(
                foundation,
                protocols.foreign_callback_protocol(),
            )?,
            source_location_protocol: ImportedCoreSourceLocationProtocol::import(
                foundation,
                protocols.source_location_protocol(),
            )?,
            program_arguments: import::import_callable(foundation, protocols.program_arguments())?,
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
        FUNDAMENTAL_TYPE_COUNT
            + OPTION_PROTOCOL_COUNT
            + ITERATION_PROTOCOL_COUNT
            + EXCEPTION_PROTOCOL_COUNT
            + COROUTINE_PROTOCOL_COUNT
            + FFI_PROTOCOL_COUNT
            + FOREIGN_CALLBACK_PROTOCOL_COUNT
            + SOURCE_LOCATION_PROTOCOL_COUNT
            + 1
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
        interface: &CoreCompilerProtocolSurfaceV1,
    ) -> Result<ImportedCoreInputs, CoreProtocolImportError> {
        let protocols = ImportedCoreProtocols::import(self, interface)?;
        Ok(ImportedCoreInputs { protocols })
    }
}

#[cfg(test)]
mod tests;
