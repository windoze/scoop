//! Resolve each fixed role into its imported identity session.

use super::*;

impl ImportedCoreFundamentalTypeProtocol {
    pub(in crate::foundation::imported_protocols) fn import(
        foundation: &ImportedHirFoundation,
        protocol: &crate::CoreFundamentalTypeProtocolV1,
    ) -> Result<Self, CoreProtocolImportError> {
        Ok(Self {
            unit: foundation
                .source_nominal(protocol.unit)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.unit))?,
            int8: foundation
                .source_nominal(protocol.int8)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.int8))?,
            int16: foundation
                .source_nominal(protocol.int16)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.int16))?,
            int32: foundation
                .source_nominal(protocol.int32)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.int32))?,
            int64: foundation
                .source_nominal(protocol.int64)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.int64))?,
            uint8: foundation
                .source_nominal(protocol.uint8)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.uint8))?,
            uint16: foundation
                .source_nominal(protocol.uint16)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.uint16))?,
            uint32: foundation
                .source_nominal(protocol.uint32)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.uint32))?,
            uint64: foundation
                .source_nominal(protocol.uint64)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.uint64))?,
            boolean: foundation
                .source_nominal(protocol.boolean)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.boolean))?,
            string: foundation
                .source_nominal(protocol.string)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.string))?,
            array: foundation
                .generic_nominal(protocol.array)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::GenericType, protocol.array))?,
            mutable_array: foundation
                .generic_nominal(protocol.mutable_array)
                .ok_or_else(|| {
                    missing(
                        CoreProtocolIdentityKind::GenericType,
                        protocol.mutable_array,
                    )
                })?,
            ptr: foundation
                .generic_nominal(protocol.ptr)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::GenericType, protocol.ptr))?,
            fun_ptr: foundation
                .generic_nominal(protocol.fun_ptr)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::GenericType, protocol.fun_ptr))?,
            character: foundation
                .source_nominal(protocol.character)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.character))?,
            float: foundation
                .source_nominal(protocol.float)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.float))?,
            double: foundation
                .source_nominal(protocol.double)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.double))?,
            any: foundation
                .source_nominal(protocol.any)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.any))?,
            nothing: foundation
                .source_nominal(protocol.nothing)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.nothing))?,
        })
    }
}

impl ImportedCoreOptionProtocol {
    pub(in crate::foundation::imported_protocols) fn import(
        foundation: &ImportedHirFoundation,
        protocol: &crate::CoreOptionProtocolV1,
    ) -> Result<Self, CoreProtocolImportError> {
        Ok(Self {
            option: foundation
                .generic_nominal(protocol.option)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::GenericType, protocol.option))?,
            some: foundation
                .identity(protocol.some)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::EnumVariant, protocol.some))?,
            some_payload: foundation.identity(protocol.some_payload).ok_or_else(|| {
                missing(
                    CoreProtocolIdentityKind::EnumVariantField,
                    protocol.some_payload,
                )
            })?,
            none: foundation
                .identity(protocol.none)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::EnumVariant, protocol.none))?,
        })
    }
}

impl ImportedCoreIterationProtocol {
    pub(in crate::foundation::imported_protocols) fn import(
        foundation: &ImportedHirFoundation,
        protocol: &crate::CoreIterationProtocolV1,
    ) -> Result<Self, CoreProtocolImportError> {
        Ok(Self {
            iterator: foundation
                .generic_nominal(protocol.iterator)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::GenericType, protocol.iterator))?,
            next: import_callable(foundation, &protocol.next)?,
            next_dispatch: foundation.identity(protocol.next_dispatch).ok_or_else(|| {
                missing(
                    CoreProtocolIdentityKind::DispatchSlot,
                    protocol.next_dispatch,
                )
            })?,
        })
    }
}

impl ImportedCoreExceptionProtocol {
    pub(in crate::foundation::imported_protocols) fn import(
        foundation: &ImportedHirFoundation,
        protocol: &crate::CoreExceptionProtocolV1,
    ) -> Result<Self, CoreProtocolImportError> {
        Ok(Self {
            throwable: foundation
                .source_nominal(protocol.throwable)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.throwable))?,
            throwable_constructor: import_callable(foundation, &protocol.throwable_constructor)?,
            unwrap_exception: foundation
                .source_nominal(protocol.unwrap_exception)
                .ok_or_else(|| {
                    missing(CoreProtocolIdentityKind::Type, protocol.unwrap_exception)
                })?,
            unwrap_exception_constructor: import_callable(
                foundation,
                &protocol.unwrap_exception_constructor,
            )?,
            class_cast_exception: foundation
                .source_nominal(protocol.class_cast_exception)
                .ok_or_else(|| {
                    missing(
                        CoreProtocolIdentityKind::Type,
                        protocol.class_cast_exception,
                    )
                })?,
            class_cast_exception_constructor: import_callable(
                foundation,
                &protocol.class_cast_exception_constructor,
            )?,
            arithmetic_exception: foundation
                .source_nominal(protocol.arithmetic_exception)
                .ok_or_else(|| {
                    missing(
                        CoreProtocolIdentityKind::Type,
                        protocol.arithmetic_exception,
                    )
                })?,
            arithmetic_exception_constructor: import_callable(
                foundation,
                &protocol.arithmetic_exception_constructor,
            )?,
            index_out_of_bounds_exception: foundation
                .source_nominal(protocol.index_out_of_bounds_exception)
                .ok_or_else(|| {
                    missing(
                        CoreProtocolIdentityKind::Type,
                        protocol.index_out_of_bounds_exception,
                    )
                })?,
            index_out_of_bounds_exception_constructor: import_callable(
                foundation,
                &protocol.index_out_of_bounds_exception_constructor,
            )?,
            illegal_state_exception: foundation
                .source_nominal(protocol.illegal_state_exception)
                .ok_or_else(|| {
                    missing(
                        CoreProtocolIdentityKind::Type,
                        protocol.illegal_state_exception,
                    )
                })?,
            illegal_state_exception_constructor: import_callable(
                foundation,
                &protocol.illegal_state_exception_constructor,
            )?,
            initialization_cycle_thrower: import_callable(
                foundation,
                &protocol.initialization_cycle_thrower,
            )?,
            illegal_argument_exception: foundation
                .source_nominal(protocol.illegal_argument_exception)
                .ok_or_else(|| {
                    missing(
                        CoreProtocolIdentityKind::Type,
                        protocol.illegal_argument_exception,
                    )
                })?,
            illegal_argument_exception_constructor: import_callable(
                foundation,
                &protocol.illegal_argument_exception_constructor,
            )?,
            missing_context_exception: foundation
                .source_nominal(protocol.missing_context_exception)
                .ok_or_else(|| {
                    missing(
                        CoreProtocolIdentityKind::Type,
                        protocol.missing_context_exception,
                    )
                })?,
            missing_context_exception_constructor: import_callable(
                foundation,
                &protocol.missing_context_exception_constructor,
            )?,
        })
    }
}

impl ImportedCoreCoroutineProtocol {
    pub(in crate::foundation::imported_protocols) fn import(
        foundation: &ImportedHirFoundation,
        protocol: &crate::CoreCoroutineProtocolV1,
    ) -> Result<Self, CoreProtocolImportError> {
        Ok(Self {
            continuation: foundation
                .generic_nominal(protocol.continuation)
                .ok_or_else(|| {
                    missing(CoreProtocolIdentityKind::GenericType, protocol.continuation)
                })?,
            continuation_resume: import_callable(foundation, &protocol.continuation_resume)?,
            continuation_resume_dispatch: foundation
                .identity(protocol.continuation_resume_dispatch)
                .ok_or_else(|| {
                    missing(
                        CoreProtocolIdentityKind::DispatchSlot,
                        protocol.continuation_resume_dispatch,
                    )
                })?,
            continuation_resume_with_exception: import_callable(
                foundation,
                &protocol.continuation_resume_with_exception,
            )?,
            continuation_resume_with_exception_dispatch: foundation
                .identity(protocol.continuation_resume_with_exception_dispatch)
                .ok_or_else(|| {
                    missing(
                        CoreProtocolIdentityKind::DispatchSlot,
                        protocol.continuation_resume_with_exception_dispatch,
                    )
                })?,
            suspend_task: foundation
                .generic_nominal(protocol.suspend_task)
                .ok_or_else(|| {
                    missing(CoreProtocolIdentityKind::GenericType, protocol.suspend_task)
                })?,
            suspend_task_run: import_callable(foundation, &protocol.suspend_task_run)?,
            suspend_task_run_dispatch: foundation
                .identity(protocol.suspend_task_run_dispatch)
                .ok_or_else(|| {
                    missing(
                        CoreProtocolIdentityKind::DispatchSlot,
                        protocol.suspend_task_run_dispatch,
                    )
                })?,
            suspend_registration: foundation
                .generic_nominal(protocol.suspend_registration)
                .ok_or_else(|| {
                    missing(
                        CoreProtocolIdentityKind::GenericType,
                        protocol.suspend_registration,
                    )
                })?,
            suspend_registration_register: import_callable(
                foundation,
                &protocol.suspend_registration_register,
            )?,
            suspend_registration_register_dispatch: foundation
                .identity(protocol.suspend_registration_register_dispatch)
                .ok_or_else(|| {
                    missing(
                        CoreProtocolIdentityKind::DispatchSlot,
                        protocol.suspend_registration_register_dispatch,
                    )
                })?,
            start_coroutine: import_callable(foundation, &protocol.start_coroutine)?,
            suspend_coroutine: import_callable(foundation, &protocol.suspend_coroutine)?,
        })
    }
}

impl ImportedCoreFfiProtocol {
    pub(in crate::foundation::imported_protocols) fn import(
        foundation: &ImportedHirFoundation,
        protocol: &crate::CoreFfiProtocolV1,
    ) -> Result<Self, CoreProtocolImportError> {
        Ok(Self {
            ptr: foundation
                .generic_nominal(protocol.ptr)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::GenericType, protocol.ptr))?,
            fun_ptr: foundation
                .generic_nominal(protocol.fun_ptr)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::GenericType, protocol.fun_ptr))?,
            pinned_ptr: foundation
                .generic_nominal(protocol.pinned_ptr)
                .ok_or_else(|| {
                    missing(CoreProtocolIdentityKind::GenericType, protocol.pinned_ptr)
                })?,
            gc_handle: foundation
                .generic_nominal(protocol.gc_handle)
                .ok_or_else(|| {
                    missing(CoreProtocolIdentityKind::GenericType, protocol.gc_handle)
                })?,
            ptr_to_ulong: import_callable(foundation, &protocol.ptr_to_ulong)?,
            ptr_cast: import_callable(foundation, &protocol.ptr_cast)?,
            ptr_load: import_callable(foundation, &protocol.ptr_load)?,
            ptr_load_offset: import_callable(foundation, &protocol.ptr_load_offset)?,
            ptr_store: import_callable(foundation, &protocol.ptr_store)?,
            ptr_store_offset: import_callable(foundation, &protocol.ptr_store_offset)?,
            ptr_plus: import_callable(foundation, &protocol.ptr_plus)?,
            ptr_minus: import_callable(foundation, &protocol.ptr_minus)?,
            address_of: import_callable(foundation, &protocol.address_of)?,
            size_of: import_callable(foundation, &protocol.size_of)?,
            align_of: import_callable(foundation, &protocol.align_of)?,
            gc_pin_raw: import_callable(foundation, &protocol.gc_pin_raw)?,
            gc_unpin_raw: import_callable(foundation, &protocol.gc_unpin_raw)?,
            gc_get_handle_raw: import_callable(foundation, &protocol.gc_get_handle_raw)?,
            gc_release_handle_raw: import_callable(foundation, &protocol.gc_release_handle_raw)?,
        })
    }
}

impl ImportedCoreForeignCallbackProtocol {
    pub(in crate::foundation::imported_protocols) fn import(
        foundation: &ImportedHirFoundation,
        protocol: &crate::CoreForeignCallbackProtocolV1,
    ) -> Result<Self, CoreProtocolImportError> {
        Ok(Self {
            callback: foundation
                .generic_nominal(protocol.callback)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::GenericType, protocol.callback))?,
            mode: foundation
                .source_nominal(protocol.mode)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.mode))?,
            reusable: foundation
                .identity(protocol.reusable)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::EnumVariant, protocol.reusable))?,
            one_shot: foundation
                .identity(protocol.one_shot)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::EnumVariant, protocol.one_shot))?,
            state: foundation
                .source_nominal(protocol.state)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.state))?,
            registered: foundation.identity(protocol.registered).ok_or_else(|| {
                missing(CoreProtocolIdentityKind::EnumVariant, protocol.registered)
            })?,
            active: foundation
                .identity(protocol.active)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::EnumVariant, protocol.active))?,
            completed: foundation.identity(protocol.completed).ok_or_else(|| {
                missing(CoreProtocolIdentityKind::EnumVariant, protocol.completed)
            })?,
            failed: foundation
                .identity(protocol.failed)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::EnumVariant, protocol.failed))?,
            failure_result: foundation
                .identity(protocol.failure_result)
                .ok_or_else(|| {
                    missing(CoreProtocolIdentityKind::ExactType, protocol.failure_result)
                })?,
            register: import_callable(foundation, &protocol.register)?,
            retain: import_callable(foundation, &protocol.retain)?,
            release: import_callable(foundation, &protocol.release)?,
            query_state: import_callable(foundation, &protocol.query_state)?,
            failure: import_callable(foundation, &protocol.failure)?,
        })
    }
}

impl ImportedCoreSourceLocationProtocol {
    pub(in crate::foundation::imported_protocols) fn import(
        foundation: &ImportedHirFoundation,
        protocol: &crate::CoreSourceLocationProtocolV1,
    ) -> Result<Self, CoreProtocolImportError> {
        Ok(Self {
            location: foundation
                .source_nominal(protocol.location)
                .ok_or_else(|| missing(CoreProtocolIdentityKind::Type, protocol.location))?,
            current: import_callable(foundation, &protocol.current)?,
        })
    }
}
