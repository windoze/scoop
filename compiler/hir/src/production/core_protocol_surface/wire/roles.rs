//! Keep the established wire role order while decoding directly to named fields.

use super::*;

impl CoreFundamentalTypeProtocolV1 {
    pub(in crate::production::core_protocol_surface) fn wire_entries(
        &self,
    ) -> [CoreProtocolEntryV1; FUNDAMENTAL_TYPE_COUNT] {
        [
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.unit)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.int8)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.int16)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.int32)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.int64)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.uint8)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.uint16)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.uint32)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.uint64)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.boolean)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.string)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(self.array)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(self.mutable_array)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(self.ptr)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(self.fun_ptr)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.character)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.float)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.double)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.any)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.nothing)),
        ]
    }
}

impl DecodedCoreFundamentalTypeProtocolV1 {
    pub(super) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreFundamentalTypeProtocolV1, CoreCompilerProtocolSurfaceValidationError> {
        let [
            unit,
            int8,
            int16,
            int32,
            int64,
            uint8,
            uint16,
            uint32,
            uint64,
            boolean,
            string,
            array,
            mutable_array,
            ptr,
            fun_ptr,
            character,
            float,
            double,
            any,
            nothing,
        ] = self.0.entries;
        Ok(CoreFundamentalTypeProtocolV1 {
            unit: read_concrete_nominal(unit, 0, foundation)?,
            int8: read_concrete_nominal(int8, 1, foundation)?,
            int16: read_concrete_nominal(int16, 2, foundation)?,
            int32: read_concrete_nominal(int32, 3, foundation)?,
            int64: read_concrete_nominal(int64, 4, foundation)?,
            uint8: read_concrete_nominal(uint8, 5, foundation)?,
            uint16: read_concrete_nominal(uint16, 6, foundation)?,
            uint32: read_concrete_nominal(uint32, 7, foundation)?,
            uint64: read_concrete_nominal(uint64, 8, foundation)?,
            boolean: read_concrete_nominal(boolean, 9, foundation)?,
            string: read_concrete_nominal(string, 10, foundation)?,
            array: read_generic_nominal(array, 11, foundation)?,
            mutable_array: read_generic_nominal(mutable_array, 12, foundation)?,
            ptr: read_generic_nominal(ptr, 13, foundation)?,
            fun_ptr: read_generic_nominal(fun_ptr, 14, foundation)?,
            character: read_concrete_nominal(character, 15, foundation)?,
            float: read_concrete_nominal(float, 16, foundation)?,
            double: read_concrete_nominal(double, 17, foundation)?,
            any: read_concrete_nominal(any, 18, foundation)?,
            nothing: read_concrete_nominal(nothing, 19, foundation)?,
        })
    }
}

impl CoreOptionProtocolV1 {
    pub(in crate::production::core_protocol_surface) fn wire_entries(
        &self,
    ) -> [CoreProtocolEntryV1; OPTION_PROTOCOL_COUNT] {
        [
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(self.option)),
            CoreProtocolEntryV1::EnumVariant(self.some),
            CoreProtocolEntryV1::EnumVariantField(self.some_payload),
            CoreProtocolEntryV1::EnumVariant(self.none),
        ]
    }
}

impl DecodedCoreOptionProtocolV1 {
    pub(super) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreOptionProtocolV1, CoreCompilerProtocolSurfaceValidationError> {
        let [option, some, some_payload, none] = self.0.entries;
        Ok(CoreOptionProtocolV1 {
            option: read_generic_nominal(option, 0, foundation)?,
            some: read_enum_variant(some, 1, foundation)?,
            some_payload: read_enum_variant_field(some_payload, 2, foundation)?,
            none: read_enum_variant(none, 3, foundation)?,
        })
    }
}

impl CoreIterationProtocolV1 {
    pub(in crate::production::core_protocol_surface) fn wire_entries(
        &self,
    ) -> [CoreProtocolEntryV1; ITERATION_PROTOCOL_COUNT] {
        [
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(self.iterator)),
            CoreProtocolEntryV1::Callable(self.next.clone()),
            CoreProtocolEntryV1::DispatchSlot(self.next_dispatch),
        ]
    }
}

impl DecodedCoreIterationProtocolV1 {
    pub(super) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreIterationProtocolV1, CoreCompilerProtocolSurfaceValidationError> {
        let [iterator, next, next_dispatch] = self.0.entries;
        Ok(CoreIterationProtocolV1 {
            iterator: read_generic_nominal(iterator, 0, foundation)?,
            next: read_callable(next, 1, foundation)?,
            next_dispatch: read_dispatch_slot(next_dispatch, 2, foundation)?,
        })
    }
}

impl CoreExceptionProtocolV1 {
    pub(in crate::production::core_protocol_surface) fn wire_entries(
        &self,
    ) -> [CoreProtocolEntryV1; EXCEPTION_PROTOCOL_COUNT] {
        [
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.throwable)),
            CoreProtocolEntryV1::Callable(self.throwable_constructor.clone()),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.unwrap_exception)),
            CoreProtocolEntryV1::Callable(self.unwrap_exception_constructor.clone()),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.class_cast_exception)),
            CoreProtocolEntryV1::Callable(self.class_cast_exception_constructor.clone()),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.arithmetic_exception)),
            CoreProtocolEntryV1::Callable(self.arithmetic_exception_constructor.clone()),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(
                self.index_out_of_bounds_exception,
            )),
            CoreProtocolEntryV1::Callable(self.index_out_of_bounds_exception_constructor.clone()),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.illegal_state_exception)),
            CoreProtocolEntryV1::Callable(self.illegal_state_exception_constructor.clone()),
            CoreProtocolEntryV1::Callable(self.initialization_cycle_thrower.clone()),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(
                self.illegal_argument_exception,
            )),
            CoreProtocolEntryV1::Callable(self.illegal_argument_exception_constructor.clone()),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(
                self.missing_context_exception,
            )),
            CoreProtocolEntryV1::Callable(self.missing_context_exception_constructor.clone()),
        ]
    }
}

impl DecodedCoreExceptionProtocolV1 {
    pub(super) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreExceptionProtocolV1, CoreCompilerProtocolSurfaceValidationError> {
        let [
            throwable,
            throwable_constructor,
            unwrap_exception,
            unwrap_exception_constructor,
            class_cast_exception,
            class_cast_exception_constructor,
            arithmetic_exception,
            arithmetic_exception_constructor,
            index_out_of_bounds_exception,
            index_out_of_bounds_exception_constructor,
            illegal_state_exception,
            illegal_state_exception_constructor,
            initialization_cycle_thrower,
            illegal_argument_exception,
            illegal_argument_exception_constructor,
            missing_context_exception,
            missing_context_exception_constructor,
        ] = self.0.entries;
        Ok(CoreExceptionProtocolV1 {
            throwable: read_concrete_nominal(throwable, 0, foundation)?,
            throwable_constructor: read_callable(throwable_constructor, 1, foundation)?,
            unwrap_exception: read_concrete_nominal(unwrap_exception, 2, foundation)?,
            unwrap_exception_constructor: read_callable(
                unwrap_exception_constructor,
                3,
                foundation,
            )?,
            class_cast_exception: read_concrete_nominal(class_cast_exception, 4, foundation)?,
            class_cast_exception_constructor: read_callable(
                class_cast_exception_constructor,
                5,
                foundation,
            )?,
            arithmetic_exception: read_concrete_nominal(arithmetic_exception, 6, foundation)?,
            arithmetic_exception_constructor: read_callable(
                arithmetic_exception_constructor,
                7,
                foundation,
            )?,
            index_out_of_bounds_exception: read_concrete_nominal(
                index_out_of_bounds_exception,
                8,
                foundation,
            )?,
            index_out_of_bounds_exception_constructor: read_callable(
                index_out_of_bounds_exception_constructor,
                9,
                foundation,
            )?,
            illegal_state_exception: read_concrete_nominal(
                illegal_state_exception,
                10,
                foundation,
            )?,
            illegal_state_exception_constructor: read_callable(
                illegal_state_exception_constructor,
                11,
                foundation,
            )?,
            initialization_cycle_thrower: read_callable(
                initialization_cycle_thrower,
                12,
                foundation,
            )?,
            illegal_argument_exception: read_concrete_nominal(
                illegal_argument_exception,
                13,
                foundation,
            )?,
            illegal_argument_exception_constructor: read_callable(
                illegal_argument_exception_constructor,
                14,
                foundation,
            )?,
            missing_context_exception: read_concrete_nominal(
                missing_context_exception,
                15,
                foundation,
            )?,
            missing_context_exception_constructor: read_callable(
                missing_context_exception_constructor,
                16,
                foundation,
            )?,
        })
    }
}

impl CoreCoroutineProtocolV1 {
    pub(in crate::production::core_protocol_surface) fn wire_entries(
        &self,
    ) -> [CoreProtocolEntryV1; COROUTINE_PROTOCOL_COUNT] {
        [
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(self.continuation)),
            CoreProtocolEntryV1::Callable(self.continuation_resume.clone()),
            CoreProtocolEntryV1::DispatchSlot(self.continuation_resume_dispatch),
            CoreProtocolEntryV1::Callable(self.continuation_resume_with_exception.clone()),
            CoreProtocolEntryV1::DispatchSlot(self.continuation_resume_with_exception_dispatch),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(self.suspend_task)),
            CoreProtocolEntryV1::Callable(self.suspend_task_run.clone()),
            CoreProtocolEntryV1::DispatchSlot(self.suspend_task_run_dispatch),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(
                self.suspend_registration,
            )),
            CoreProtocolEntryV1::Callable(self.suspend_registration_register.clone()),
            CoreProtocolEntryV1::DispatchSlot(self.suspend_registration_register_dispatch),
            CoreProtocolEntryV1::Callable(self.start_coroutine.clone()),
            CoreProtocolEntryV1::Callable(self.suspend_coroutine.clone()),
        ]
    }
}

impl DecodedCoreCoroutineProtocolV1 {
    pub(super) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreCoroutineProtocolV1, CoreCompilerProtocolSurfaceValidationError> {
        let [
            continuation,
            continuation_resume,
            continuation_resume_dispatch,
            continuation_resume_with_exception,
            continuation_resume_with_exception_dispatch,
            suspend_task,
            suspend_task_run,
            suspend_task_run_dispatch,
            suspend_registration,
            suspend_registration_register,
            suspend_registration_register_dispatch,
            start_coroutine,
            suspend_coroutine,
        ] = self.0.entries;
        Ok(CoreCoroutineProtocolV1 {
            continuation: read_generic_nominal(continuation, 0, foundation)?,
            continuation_resume: read_callable(continuation_resume, 1, foundation)?,
            continuation_resume_dispatch: read_dispatch_slot(
                continuation_resume_dispatch,
                2,
                foundation,
            )?,
            continuation_resume_with_exception: read_callable(
                continuation_resume_with_exception,
                3,
                foundation,
            )?,
            continuation_resume_with_exception_dispatch: read_dispatch_slot(
                continuation_resume_with_exception_dispatch,
                4,
                foundation,
            )?,
            suspend_task: read_generic_nominal(suspend_task, 5, foundation)?,
            suspend_task_run: read_callable(suspend_task_run, 6, foundation)?,
            suspend_task_run_dispatch: read_dispatch_slot(
                suspend_task_run_dispatch,
                7,
                foundation,
            )?,
            suspend_registration: read_generic_nominal(suspend_registration, 8, foundation)?,
            suspend_registration_register: read_callable(
                suspend_registration_register,
                9,
                foundation,
            )?,
            suspend_registration_register_dispatch: read_dispatch_slot(
                suspend_registration_register_dispatch,
                10,
                foundation,
            )?,
            start_coroutine: read_callable(start_coroutine, 11, foundation)?,
            suspend_coroutine: read_callable(suspend_coroutine, 12, foundation)?,
        })
    }
}

mod ffi;
