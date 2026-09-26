//! Complete typed declaration roles required by compiler protocols.

use scoop_identity::{
    CoreBuiltinNominal, PersistentDispatchSlotId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExactTypeId, PersistentGenericTypeId, PersistentTypeId,
};

use super::{CoreProtocolCallableBuildError, CoreProtocolCallableV1};
use crate::{
    ClassId, EnumId, EnumVariantFieldRef, EnumVariantRef, ExportHir, FunctionId,
    HirNominalIdentity, HirSourceNominalIdentity, InterfaceId, InterfaceMethodId,
    IntrinsicFunctionKind, MethodDispatch, StructId, TypeId,
};

mod fixed_signatures;
mod operation_owners;
mod operation_signatures;
mod shared_intrinsics;
mod source_types;
use fixed_signatures::validate_fixed_callable_signatures;
use operation_owners::expected_operation_owner;
pub use shared_intrinsics::IntrinsicCallableContractError;
mod validation;
mod wire;
use operation_signatures::expected_operation_signature;
use operation_signatures::operation_own_type_parameter_count;
pub use validation::{
    CoreCompilerProtocolSurfaceBuildError, CoreCompilerProtocolSurfaceRelationError,
    CoreProtocolProductKindV1,
};
pub use wire::{CoreCompilerProtocolSurfaceValidationError, DecodedCoreCompilerProtocolSurfaceV1};
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;

pub(crate) const FUNDAMENTAL_TYPE_COUNT: usize = 15;
pub(crate) const OPTION_PROTOCOL_COUNT: usize = 4;
pub(crate) const ITERATION_PROTOCOL_COUNT: usize = 3;
pub(crate) const EXCEPTION_PROTOCOL_COUNT: usize = 13;
pub(crate) const COROUTINE_PROTOCOL_COUNT: usize = 13;
pub(crate) const FFI_PROTOCOL_COUNT: usize = 19;
pub(crate) const FOREIGN_CALLBACK_PROTOCOL_COUNT: usize = 15;
pub(crate) const SOURCE_LOCATION_PROTOCOL_COUNT: usize = 2;

/// Persistent nominal identity without an arena-id or FQN fallback.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CoreProtocolNominalV1 {
    Type(PersistentTypeId),
    GenericType(PersistentGenericTypeId),
}

/// Closed subject sum used by the fixed-role protocol products.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CoreProtocolEntryV1 {
    Nominal(CoreProtocolNominalV1),
    Callable(CoreProtocolCallableV1),
    EnumVariant(PersistentEnumVariantId),
    EnumVariantField(PersistentEnumVariantFieldId),
    DispatchSlot(PersistentDispatchSlotId),
    ExactType(PersistentExactTypeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreProtocolProductV1<const N: usize> {
    entries: [CoreProtocolEntryV1; N],
}

macro_rules! protocol_product {
    ($name:ident, $count:ident) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $name(CoreProtocolProductV1<$count>);

        impl $name {
            pub(crate) fn entries(&self) -> &[CoreProtocolEntryV1; $count] {
                &self.0.entries
            }
        }
    };
}

protocol_product!(CoreFundamentalTypeProtocolV1, FUNDAMENTAL_TYPE_COUNT);
protocol_product!(CoreOptionProtocolV1, OPTION_PROTOCOL_COUNT);
protocol_product!(CoreIterationProtocolV1, ITERATION_PROTOCOL_COUNT);
protocol_product!(CoreExceptionProtocolV1, EXCEPTION_PROTOCOL_COUNT);
protocol_product!(CoreCoroutineProtocolV1, COROUTINE_PROTOCOL_COUNT);
protocol_product!(CoreFfiProtocolV1, FFI_PROTOCOL_COUNT);
protocol_product!(
    CoreForeignCallbackProtocolV1,
    FOREIGN_CALLBACK_PROTOCOL_COUNT
);
protocol_product!(CoreSourceLocationProtocolV1, SOURCE_LOCATION_PROTOCOL_COUNT);

/// Complete typed declaration references for compiler protocols. Every
/// constituent is a closed product and the eight products are validated as
/// one complete set of declaration references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreCompilerProtocolSurfaceV1 {
    fundamental_types: CoreFundamentalTypeProtocolV1,
    option_protocol: CoreOptionProtocolV1,
    iteration_protocol: CoreIterationProtocolV1,
    exception_protocol: CoreExceptionProtocolV1,
    coroutine_protocol: CoreCoroutineProtocolV1,
    ffi_protocol: CoreFfiProtocolV1,
    foreign_callback_protocol: CoreForeignCallbackProtocolV1,
    source_location_protocol: CoreSourceLocationProtocolV1,
}

impl CoreCompilerProtocolSurfaceV1 {
    pub fn from_export(
        export: &ExportHir,
        protocols: &crate::DefinedCoreProtocols,
    ) -> Result<Self, CoreCompilerProtocolSurfaceBuildError> {
        let fundamental_types = CoreFundamentalTypeProtocolV1(product([
            concrete(CoreProtocolNominalV1::Type(
                CoreBuiltinNominal::Unit.identity_record().id(),
            ))?,
            concrete(integer(export, protocols, crate::IntegerKind::SIGNED_8)?)?,
            concrete(integer(export, protocols, crate::IntegerKind::SIGNED_16)?)?,
            concrete(integer(export, protocols, crate::IntegerKind::SIGNED_32)?)?,
            concrete(integer(export, protocols, crate::IntegerKind::SIGNED_64)?)?,
            concrete(integer(export, protocols, crate::IntegerKind::UNSIGNED_8)?)?,
            concrete(integer(export, protocols, crate::IntegerKind::UNSIGNED_16)?)?,
            concrete(integer(export, protocols, crate::IntegerKind::UNSIGNED_32)?)?,
            concrete(integer(export, protocols, crate::IntegerKind::UNSIGNED_64)?)?,
            concrete(struct_nominal(export, protocols.fundamental_types.boolean)?)?,
            concrete(class_nominal(export, protocols.fundamental_types.string)?)?,
            generic(class_nominal(export, protocols.fundamental_types.array)?)?,
            generic(class_nominal(
                export,
                protocols.fundamental_types.mutable_array,
            )?)?,
            generic(struct_nominal(export, protocols.fundamental_types.ptr)?)?,
            generic(struct_nominal(export, protocols.fundamental_types.fun_ptr)?)?,
        ]));

        let option = protocols.option;
        let option_protocol = CoreOptionProtocolV1(product([
            generic(enum_nominal(export, option.enumeration())?)?,
            variant(export, option.some()),
            variant_field(export, option.some_payload()),
            variant(export, option.none()),
        ]));

        let iteration = protocols.iteration;
        let iteration_protocol = CoreIterationProtocolV1(product([
            generic(interface_nominal(export, iteration.iterator())?)?,
            callable(
                export,
                protocols,
                export.interface_methods[iteration.next()].function,
            )?,
            dispatch_slot(export, iteration.next()),
        ]));

        let exceptions = protocols.exceptions;
        let exception_protocol = CoreExceptionProtocolV1(product([
            concrete(class_nominal(export, exceptions.throwable.class())?)?,
            constructor(export, protocols, exceptions.throwable.callable())?,
            concrete(class_nominal(export, exceptions.unwrap_exception.class())?)?,
            constructor(export, protocols, exceptions.unwrap_exception.callable())?,
            concrete(class_nominal(
                export,
                exceptions.class_cast_exception.class(),
            )?)?,
            constructor(
                export,
                protocols,
                exceptions.class_cast_exception.callable(),
            )?,
            concrete(class_nominal(
                export,
                exceptions.arithmetic_exception.class(),
            )?)?,
            constructor(
                export,
                protocols,
                exceptions.arithmetic_exception.callable(),
            )?,
            concrete(class_nominal(
                export,
                exceptions.index_out_of_bounds_exception.class(),
            )?)?,
            constructor(
                export,
                protocols,
                exceptions.index_out_of_bounds_exception.callable(),
            )?,
            concrete(class_nominal(
                export,
                exceptions.illegal_state_exception.class(),
            )?)?,
            constructor(
                export,
                protocols,
                exceptions.illegal_state_exception.callable(),
            )?,
            callable(export, protocols, exceptions.initialization_cycle_thrower)?,
        ]));

        let coroutine = protocols.coroutines;
        let coroutine_protocol = CoreCoroutineProtocolV1(product([
            generic(interface_nominal(export, coroutine.continuation)?)?,
            callable(export, protocols, coroutine.continuation_resume)?,
            function_dispatch_slot(export, coroutine.continuation_resume)?,
            callable(
                export,
                protocols,
                coroutine.continuation_resume_with_exception,
            )?,
            function_dispatch_slot(export, coroutine.continuation_resume_with_exception)?,
            generic(interface_nominal(export, coroutine.suspend_task)?)?,
            callable(export, protocols, coroutine.suspend_task_run)?,
            function_dispatch_slot(export, coroutine.suspend_task_run)?,
            generic(interface_nominal(export, coroutine.suspend_registration)?)?,
            callable(export, protocols, coroutine.suspend_registration_register)?,
            function_dispatch_slot(export, coroutine.suspend_registration_register)?,
            callable(export, protocols, coroutine.start_coroutine)?,
            callable(export, protocols, coroutine.suspend_coroutine)?,
        ]));

        let ffi = protocols.ffi;
        let ffi_protocol = CoreFfiProtocolV1(product([
            generic(struct_nominal(export, ffi.ptr)?)?,
            generic(struct_nominal(export, ffi.fun_ptr)?)?,
            generic(struct_nominal(export, ffi.pinned_ptr)?)?,
            generic(struct_nominal(export, ffi.gc_handle)?)?,
            callable(export, protocols, ffi.ptr_to_ulong)?,
            callable(export, protocols, ffi.ptr_cast)?,
            callable(export, protocols, ffi.ptr_load)?,
            callable(export, protocols, ffi.ptr_load_offset)?,
            callable(export, protocols, ffi.ptr_store)?,
            callable(export, protocols, ffi.ptr_store_offset)?,
            callable(export, protocols, ffi.ptr_plus)?,
            callable(export, protocols, ffi.ptr_minus)?,
            callable(export, protocols, ffi.address_of)?,
            callable(export, protocols, ffi.size_of)?,
            callable(export, protocols, ffi.align_of)?,
            callable(export, protocols, ffi.gc_pin_raw)?,
            callable(export, protocols, ffi.gc_unpin_raw)?,
            callable(export, protocols, ffi.gc_get_handle_raw)?,
            callable(export, protocols, ffi.gc_release_handle_raw)?,
        ]));

        let callback = protocols.foreign_callbacks;
        let callback_mode = callback.modes;
        let callback_state = callback.states;
        let failure_type =
            export.enum_applications[callback.failure_result.application()].canonical_type;
        let foreign_callback_protocol = CoreForeignCallbackProtocolV1(product([
            generic(struct_nominal(export, callback.callback)?)?,
            concrete(enum_nominal(export, callback_mode.enumeration())?)?,
            applied_variant(export, callback_mode.reusable()),
            applied_variant(export, callback_mode.one_shot()),
            concrete(enum_nominal(export, callback_state.enumeration())?)?,
            applied_variant(export, callback_state.registered()),
            applied_variant(export, callback_state.active()),
            applied_variant(export, callback_state.completed()),
            applied_variant(export, callback_state.failed()),
            exact_type(export, failure_type)?,
            callable(export, protocols, callback.register)?,
            callable(export, protocols, callback.retain)?,
            callable(export, protocols, callback.release)?,
            callable(export, protocols, callback.query_state)?,
            callable(export, protocols, callback.failure)?,
        ]));

        let location = protocols.source_location;
        let source_location_protocol = CoreSourceLocationProtocolV1(product([
            concrete(struct_nominal(export, location.location)?)?,
            callable(export, protocols, location.current)?,
        ]));

        let surface = Self {
            fundamental_types,
            option_protocol,
            iteration_protocol,
            exception_protocol,
            coroutine_protocol,
            ffi_protocol,
            foreign_callback_protocol,
            source_location_protocol,
        };
        surface
            .validate_internal_relations()
            .map_err(CoreCompilerProtocolSurfaceBuildError::Relation)?;
        validate_fixed_callable_signatures(&surface)
            .map_err(CoreCompilerProtocolSurfaceBuildError::Relation)?;
        surface
            .validate_fixed_intrinsic_signatures()
            .map_err(CoreCompilerProtocolSurfaceBuildError::Relation)?;
        Ok(surface)
    }

    pub const fn fundamental_types(&self) -> &CoreFundamentalTypeProtocolV1 {
        &self.fundamental_types
    }

    pub const fn option_protocol(&self) -> &CoreOptionProtocolV1 {
        &self.option_protocol
    }

    pub const fn iteration_protocol(&self) -> &CoreIterationProtocolV1 {
        &self.iteration_protocol
    }

    pub const fn exception_protocol(&self) -> &CoreExceptionProtocolV1 {
        &self.exception_protocol
    }

    pub const fn coroutine_protocol(&self) -> &CoreCoroutineProtocolV1 {
        &self.coroutine_protocol
    }

    pub const fn ffi_protocol(&self) -> &CoreFfiProtocolV1 {
        &self.ffi_protocol
    }

    pub const fn foreign_callback_protocol(&self) -> &CoreForeignCallbackProtocolV1 {
        &self.foreign_callback_protocol
    }

    pub const fn source_location_protocol(&self) -> &CoreSourceLocationProtocolV1 {
        &self.source_location_protocol
    }

    /// Compiler service used only by generated initialization
    /// cycle edges. It is deliberately independent of the public prelude.
    pub fn initialization_cycle_thrower(&self) -> &CoreProtocolCallableV1 {
        callable_entry_ref(self.exception_protocol.entries(), 12)
    }

    /// The actual declaration selected for a failed runtime cast.
    pub fn class_cast_exception_constructor(&self) -> &CoreProtocolCallableV1 {
        callable_entry_ref(self.exception_protocol.entries(), 5)
    }

    pub fn class_cast_exception_type(&self) -> PersistentTypeId {
        concrete_entry(self.exception_protocol.entries(), 4)
    }

    pub fn string_source_type(&self) -> PersistentTypeId {
        concrete_entry(self.fundamental_types.entries(), 10)
    }

    pub fn string_exact_type(&self) -> PersistentExactTypeId {
        PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(
            self.string_source_type(),
        ))
        .expect("a resolved nominal declaration has a canonical exact identity")
    }

    pub(crate) fn option_some(&self) -> PersistentEnumVariantId {
        variant_entry(self.option_protocol.entries(), 1)
    }

    pub(crate) fn option_some_payload(&self) -> PersistentEnumVariantFieldId {
        variant_field_entry(self.option_protocol.entries(), 2)
    }

    pub(crate) fn option_none(&self) -> PersistentEnumVariantId {
        variant_entry(self.option_protocol.entries(), 3)
    }
}

fn product<const N: usize>(entries: [CoreProtocolEntryV1; N]) -> CoreProtocolProductV1<N> {
    CoreProtocolProductV1 { entries }
}

fn concrete(
    nominal: CoreProtocolNominalV1,
) -> Result<CoreProtocolEntryV1, CoreCompilerProtocolSurfaceBuildError> {
    match nominal {
        CoreProtocolNominalV1::Type(_) => Ok(CoreProtocolEntryV1::Nominal(nominal)),
        CoreProtocolNominalV1::GenericType(_) => {
            Err(CoreCompilerProtocolSurfaceBuildError::ProtocolNominalKindMismatch)
        }
    }
}

fn generic(
    nominal: CoreProtocolNominalV1,
) -> Result<CoreProtocolEntryV1, CoreCompilerProtocolSurfaceBuildError> {
    match nominal {
        CoreProtocolNominalV1::GenericType(_) => Ok(CoreProtocolEntryV1::Nominal(nominal)),
        CoreProtocolNominalV1::Type(_) => {
            Err(CoreCompilerProtocolSurfaceBuildError::ProtocolNominalKindMismatch)
        }
    }
}

fn callable(
    export: &ExportHir,
    protocols: &crate::DefinedCoreProtocols,
    function: FunctionId,
) -> Result<CoreProtocolEntryV1, CoreCompilerProtocolSurfaceBuildError> {
    CoreProtocolCallableV1::from_function(export, protocols, function)
        .map(CoreProtocolEntryV1::Callable)
        .map_err(CoreCompilerProtocolSurfaceBuildError::Callable)
}

fn constructor(
    export: &ExportHir,
    protocols: &crate::DefinedCoreProtocols,
    constructor: crate::ClassConstructorId,
) -> Result<CoreProtocolEntryV1, CoreCompilerProtocolSurfaceBuildError> {
    CoreProtocolCallableV1::from_class_constructor(export, protocols, constructor)
        .map(CoreProtocolEntryV1::Callable)
        .map_err(CoreCompilerProtocolSurfaceBuildError::Callable)
}

fn variant(export: &ExportHir, variant: EnumVariantRef) -> CoreProtocolEntryV1 {
    CoreProtocolEntryV1::EnumVariant(export.enum_member_identities[variant].id())
}

fn applied_variant(
    export: &ExportHir,
    variant: crate::AppliedEnumVariantRef,
) -> CoreProtocolEntryV1 {
    self::variant(export, variant.declaration())
}

fn variant_field(export: &ExportHir, field: EnumVariantFieldRef) -> CoreProtocolEntryV1 {
    CoreProtocolEntryV1::EnumVariantField(export.enum_member_identities[field].id())
}

fn dispatch_slot(export: &ExportHir, member: InterfaceMethodId) -> CoreProtocolEntryV1 {
    CoreProtocolEntryV1::DispatchSlot(export.dispatch_slot_identities[member].id())
}

fn function_dispatch_slot(
    export: &ExportHir,
    function: FunctionId,
) -> Result<CoreProtocolEntryV1, CoreCompilerProtocolSurfaceBuildError> {
    let member = match export.functions[function]
        .method
        .map(|method| method.dispatch)
    {
        Some(MethodDispatch::Interface(member)) => member,
        _ => {
            return Err(
                CoreCompilerProtocolSurfaceBuildError::MissingInterfaceDispatch {
                    function: function.into_raw().into_u32(),
                },
            );
        }
    };
    Ok(dispatch_slot(export, member))
}

fn exact_type(
    export: &ExportHir,
    ty: TypeId,
) -> Result<CoreProtocolEntryV1, CoreCompilerProtocolSurfaceBuildError> {
    export
        .type_identities
        .get(ty)
        .and_then(crate::HirTypeIdentity::exact)
        .map(|record| CoreProtocolEntryV1::ExactType(record.id()))
        .ok_or(CoreCompilerProtocolSurfaceBuildError::OpenProtocolType {
            ty: ty.into_raw().into_u32(),
        })
}

fn integer(
    export: &ExportHir,
    protocols: &crate::DefinedCoreProtocols,
    kind: crate::IntegerKind,
) -> Result<CoreProtocolNominalV1, CoreCompilerProtocolSurfaceBuildError> {
    struct_nominal(export, protocols.fundamental_types.integers.owner(kind))
}

fn struct_nominal(
    export: &ExportHir,
    id: StructId,
) -> Result<CoreProtocolNominalV1, CoreCompilerProtocolSurfaceBuildError> {
    nominal(&export.nominal_identities[id])
}

fn enum_nominal(
    export: &ExportHir,
    id: EnumId,
) -> Result<CoreProtocolNominalV1, CoreCompilerProtocolSurfaceBuildError> {
    nominal(&export.nominal_identities[id])
}

fn class_nominal(
    export: &ExportHir,
    id: ClassId,
) -> Result<CoreProtocolNominalV1, CoreCompilerProtocolSurfaceBuildError> {
    nominal(&export.nominal_identities[id])
}

fn interface_nominal(
    export: &ExportHir,
    id: InterfaceId,
) -> Result<CoreProtocolNominalV1, CoreCompilerProtocolSurfaceBuildError> {
    nominal(&export.nominal_identities[id])
}

fn nominal(
    identity: &HirNominalIdentity,
) -> Result<CoreProtocolNominalV1, CoreCompilerProtocolSurfaceBuildError> {
    match identity {
        HirNominalIdentity::Source(HirSourceNominalIdentity::Concrete(record)) => {
            Ok(CoreProtocolNominalV1::Type(record.id()))
        }
        HirNominalIdentity::Source(HirSourceNominalIdentity::Generic(record)) => {
            Ok(CoreProtocolNominalV1::GenericType(record.id()))
        }
        HirNominalIdentity::Generated(_) => {
            Err(CoreCompilerProtocolSurfaceBuildError::GeneratedProtocolNominal)
        }
    }
}

fn concrete_entry<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> PersistentTypeId {
    match entries[index] {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(id)) => id,
        _ => unreachable!("validated core protocol role has its fixed subject kind"),
    }
}

fn generic_entry<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> PersistentGenericTypeId {
    match entries[index] {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => id,
        _ => unreachable!("validated core protocol role has its fixed subject kind"),
    }
}

fn variant_entry<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> PersistentEnumVariantId {
    match entries[index] {
        CoreProtocolEntryV1::EnumVariant(id) => id,
        _ => unreachable!("validated core protocol role has its fixed subject kind"),
    }
}

fn variant_field_entry<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> PersistentEnumVariantFieldId {
    match entries[index] {
        CoreProtocolEntryV1::EnumVariantField(id) => id,
        _ => unreachable!("validated core protocol role has its fixed subject kind"),
    }
}

#[cfg(test)]
fn callable_entry(entry: CoreProtocolEntryV1) -> CoreProtocolCallableV1 {
    match entry {
        CoreProtocolEntryV1::Callable(callable) => callable,
        _ => unreachable!("callable builder returns a callable protocol entry"),
    }
}

fn callable_entry_ref<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> &CoreProtocolCallableV1 {
    match &entries[index] {
        CoreProtocolEntryV1::Callable(callable) => callable,
        _ => unreachable!("validated core protocol role has its fixed callable subject kind"),
    }
}
