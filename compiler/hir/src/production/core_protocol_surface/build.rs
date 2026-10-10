//! Build fixed protocol roles from resolved source declarations.

use super::*;

impl CoreCompilerProtocolSurfaceV1 {
    pub fn from_export(
        export: &ExportHir,
        protocols: &crate::DefinedCoreProtocols,
    ) -> Result<Self, CoreCompilerProtocolSurfaceBuildError> {
        let fundamental_types = CoreFundamentalTypeProtocolV1 {
            unit: CoreBuiltinNominal::Unit.identity_record().id(),
            int8: concrete(integer(export, protocols, crate::IntegerKind::SIGNED_8)?)?,
            int16: concrete(integer(export, protocols, crate::IntegerKind::SIGNED_16)?)?,
            int32: concrete(integer(export, protocols, crate::IntegerKind::SIGNED_32)?)?,
            int64: concrete(integer(export, protocols, crate::IntegerKind::SIGNED_64)?)?,
            uint8: concrete(integer(export, protocols, crate::IntegerKind::UNSIGNED_8)?)?,
            uint16: concrete(integer(export, protocols, crate::IntegerKind::UNSIGNED_16)?)?,
            uint32: concrete(integer(export, protocols, crate::IntegerKind::UNSIGNED_32)?)?,
            uint64: concrete(integer(export, protocols, crate::IntegerKind::UNSIGNED_64)?)?,
            boolean: concrete(struct_nominal(export, protocols.fundamental_types.boolean)?)?,
            string: concrete(class_nominal(export, protocols.fundamental_types.string)?)?,
            array: generic(class_nominal(export, protocols.fundamental_types.array)?)?,
            mutable_array: generic(class_nominal(
                export,
                protocols.fundamental_types.mutable_array,
            )?)?,
            ptr: generic(struct_nominal(export, protocols.fundamental_types.ptr)?)?,
            fun_ptr: generic(struct_nominal(export, protocols.fundamental_types.fun_ptr)?)?,
            character: concrete(struct_nominal(
                export,
                protocols.fundamental_types.character,
            )?)?,
            float: concrete(struct_nominal(export, protocols.fundamental_types.float)?)?,
            double: concrete(struct_nominal(export, protocols.fundamental_types.double)?)?,
            any: concrete(class_nominal(export, protocols.fundamental_types.any)?)?,
            nothing: concrete(class_nominal(export, protocols.fundamental_types.nothing)?)?,
        };

        let option = protocols.option;
        let option_protocol = CoreOptionProtocolV1 {
            option: generic(enum_nominal(export, option.enumeration())?)?,
            some: variant(export, option.some()),
            some_payload: variant_field(export, option.some_payload()),
            none: variant(export, option.none()),
        };

        let iteration = protocols.iteration;
        let iteration_protocol = CoreIterationProtocolV1 {
            iterator: generic(interface_nominal(export, iteration.iterator())?)?,
            next: callable(
                export,
                protocols,
                export.interface_methods[iteration.next()].function,
            )?,
            next_dispatch: dispatch_slot(export, iteration.next()),
        };

        let exceptions = protocols.exceptions;
        let exception_protocol = CoreExceptionProtocolV1 {
            throwable: concrete(class_nominal(export, exceptions.throwable.class())?)?,
            throwable_constructor: constructor(export, protocols, exceptions.throwable.callable())?,
            unwrap_exception: concrete(class_nominal(
                export,
                exceptions.unwrap_exception.class(),
            )?)?,
            unwrap_exception_constructor: constructor(
                export,
                protocols,
                exceptions.unwrap_exception.callable(),
            )?,
            class_cast_exception: concrete(class_nominal(
                export,
                exceptions.class_cast_exception.class(),
            )?)?,
            class_cast_exception_constructor: constructor(
                export,
                protocols,
                exceptions.class_cast_exception.callable(),
            )?,
            arithmetic_exception: concrete(class_nominal(
                export,
                exceptions.arithmetic_exception.class(),
            )?)?,
            arithmetic_exception_constructor: constructor(
                export,
                protocols,
                exceptions.arithmetic_exception.callable(),
            )?,
            index_out_of_bounds_exception: concrete(class_nominal(
                export,
                exceptions.index_out_of_bounds_exception.class(),
            )?)?,
            index_out_of_bounds_exception_constructor: constructor(
                export,
                protocols,
                exceptions.index_out_of_bounds_exception.callable(),
            )?,
            illegal_state_exception: concrete(class_nominal(
                export,
                exceptions.illegal_state_exception.class(),
            )?)?,
            illegal_state_exception_constructor: constructor(
                export,
                protocols,
                exceptions.illegal_state_exception.callable(),
            )?,
            initialization_cycle_thrower: callable(
                export,
                protocols,
                exceptions.initialization_cycle_thrower,
            )?,
            illegal_argument_exception: concrete(class_nominal(
                export,
                exceptions.illegal_argument_exception.class(),
            )?)?,
            illegal_argument_exception_constructor: constructor(
                export,
                protocols,
                exceptions.illegal_argument_exception.callable(),
            )?,
            missing_context_exception: concrete(class_nominal(
                export,
                export.class_constructors[exceptions.missing_context_constructor].owner,
            )?)?,
            missing_context_exception_constructor: constructor(
                export,
                protocols,
                exceptions.missing_context_constructor,
            )?,
        };

        let coroutine = protocols.coroutines;
        let coroutine_protocol = CoreCoroutineProtocolV1 {
            continuation: generic(interface_nominal(export, coroutine.continuation)?)?,
            continuation_resume: callable(export, protocols, coroutine.continuation_resume)?,
            continuation_resume_dispatch: function_dispatch_slot(
                export,
                coroutine.continuation_resume,
            )?,
            continuation_resume_with_exception: callable(
                export,
                protocols,
                coroutine.continuation_resume_with_exception,
            )?,
            continuation_resume_with_exception_dispatch: function_dispatch_slot(
                export,
                coroutine.continuation_resume_with_exception,
            )?,
            suspend_task: generic(interface_nominal(export, coroutine.suspend_task)?)?,
            suspend_task_run: callable(export, protocols, coroutine.suspend_task_run)?,
            suspend_task_run_dispatch: function_dispatch_slot(export, coroutine.suspend_task_run)?,
            suspend_registration: generic(interface_nominal(
                export,
                coroutine.suspend_registration,
            )?)?,
            suspend_registration_register: callable(
                export,
                protocols,
                coroutine.suspend_registration_register,
            )?,
            suspend_registration_register_dispatch: function_dispatch_slot(
                export,
                coroutine.suspend_registration_register,
            )?,
            start_coroutine: callable(export, protocols, coroutine.start_coroutine)?,
            suspend_coroutine: callable(export, protocols, coroutine.suspend_coroutine)?,
        };

        let ffi = protocols.ffi;
        let ffi_protocol = CoreFfiProtocolV1 {
            ptr: generic(struct_nominal(export, ffi.ptr)?)?,
            fun_ptr: generic(struct_nominal(export, ffi.fun_ptr)?)?,
            pinned_ptr: generic(struct_nominal(export, ffi.pinned_ptr)?)?,
            gc_handle: generic(struct_nominal(export, ffi.gc_handle)?)?,
            ptr_to_ulong: callable(export, protocols, ffi.ptr_to_ulong)?,
            ptr_cast: callable(export, protocols, ffi.ptr_cast)?,
            ptr_load: callable(export, protocols, ffi.ptr_load)?,
            ptr_load_offset: callable(export, protocols, ffi.ptr_load_offset)?,
            ptr_store: callable(export, protocols, ffi.ptr_store)?,
            ptr_store_offset: callable(export, protocols, ffi.ptr_store_offset)?,
            ptr_plus: callable(export, protocols, ffi.ptr_plus)?,
            ptr_minus: callable(export, protocols, ffi.ptr_minus)?,
            address_of: callable(export, protocols, ffi.address_of)?,
            size_of: callable(export, protocols, ffi.size_of)?,
            align_of: callable(export, protocols, ffi.align_of)?,
            gc_pin_raw: callable(export, protocols, ffi.gc_pin_raw)?,
            gc_unpin_raw: callable(export, protocols, ffi.gc_unpin_raw)?,
            gc_get_handle_raw: callable(export, protocols, ffi.gc_get_handle_raw)?,
            gc_release_handle_raw: callable(export, protocols, ffi.gc_release_handle_raw)?,
        };

        let callback = protocols.foreign_callbacks;
        let callback_mode = callback.modes;
        let callback_state = callback.states;
        let failure_type =
            export.enum_applications[callback.failure_result.application()].canonical_type;
        let foreign_callback_protocol = CoreForeignCallbackProtocolV1 {
            callback: generic(struct_nominal(export, callback.callback)?)?,
            mode: concrete(enum_nominal(export, callback_mode.enumeration())?)?,
            reusable: applied_variant(export, callback_mode.reusable()),
            one_shot: applied_variant(export, callback_mode.one_shot()),
            state: concrete(enum_nominal(export, callback_state.enumeration())?)?,
            registered: applied_variant(export, callback_state.registered()),
            active: applied_variant(export, callback_state.active()),
            completed: applied_variant(export, callback_state.completed()),
            failed: applied_variant(export, callback_state.failed()),
            failure_result: exact_type(export, failure_type)?,
            register: callable(export, protocols, callback.register)?,
            retain: callable(export, protocols, callback.retain)?,
            release: callable(export, protocols, callback.release)?,
            query_state: callable(export, protocols, callback.query_state)?,
            failure: callable(export, protocols, callback.failure)?,
        };

        let location = protocols.source_location;
        let source_location_protocol = CoreSourceLocationProtocolV1 {
            location: concrete(struct_nominal(export, location.location)?)?,
            current: callable(export, protocols, location.current)?,
        };

        let program_arguments =
            CoreProtocolCallableV1::from_function(export, protocols, protocols.program_arguments)
                .map_err(CoreCompilerProtocolSurfaceBuildError::Callable)?;
        let surface = Self {
            program_arguments,
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
}

fn concrete(
    nominal: CoreProtocolNominalV1,
) -> Result<PersistentTypeId, CoreCompilerProtocolSurfaceBuildError> {
    match nominal {
        CoreProtocolNominalV1::Type(id) => Ok(id),
        CoreProtocolNominalV1::GenericType(_) => {
            Err(CoreCompilerProtocolSurfaceBuildError::ProtocolNominalKindMismatch)
        }
    }
}

fn generic(
    nominal: CoreProtocolNominalV1,
) -> Result<PersistentGenericTypeId, CoreCompilerProtocolSurfaceBuildError> {
    match nominal {
        CoreProtocolNominalV1::GenericType(id) => Ok(id),
        CoreProtocolNominalV1::Type(_) => {
            Err(CoreCompilerProtocolSurfaceBuildError::ProtocolNominalKindMismatch)
        }
    }
}

fn callable(
    export: &ExportHir,
    protocols: &crate::DefinedCoreProtocols,
    function: FunctionId,
) -> Result<CoreProtocolCallableV1, CoreCompilerProtocolSurfaceBuildError> {
    CoreProtocolCallableV1::from_function(export, protocols, function)
        .map_err(CoreCompilerProtocolSurfaceBuildError::Callable)
}

fn constructor(
    export: &ExportHir,
    protocols: &crate::DefinedCoreProtocols,
    constructor: crate::ClassConstructorId,
) -> Result<CoreProtocolCallableV1, CoreCompilerProtocolSurfaceBuildError> {
    CoreProtocolCallableV1::from_class_constructor(export, protocols, constructor)
        .map_err(CoreCompilerProtocolSurfaceBuildError::Callable)
}

fn variant(export: &ExportHir, variant: EnumVariantRef) -> PersistentEnumVariantId {
    export.enum_member_identities[variant].id()
}

fn applied_variant(
    export: &ExportHir,
    variant: crate::AppliedEnumVariantRef,
) -> PersistentEnumVariantId {
    self::variant(export, variant.declaration())
}

fn variant_field(export: &ExportHir, field: EnumVariantFieldRef) -> PersistentEnumVariantFieldId {
    export.enum_member_identities[field].id()
}

fn dispatch_slot(export: &ExportHir, member: InterfaceMethodId) -> PersistentDispatchSlotId {
    export.dispatch_slot_identities[member].id()
}

fn function_dispatch_slot(
    export: &ExportHir,
    function: FunctionId,
) -> Result<PersistentDispatchSlotId, CoreCompilerProtocolSurfaceBuildError> {
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
) -> Result<PersistentExactTypeId, CoreCompilerProtocolSurfaceBuildError> {
    export
        .type_identities
        .get(ty)
        .and_then(crate::HirTypeIdentity::exact)
        .map(|record| record.id())
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
