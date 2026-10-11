//! Typed access to the frontend language roles.

use super::*;

impl ImportedCoreFundamentalTypeProtocol {
    pub fn any(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.any
    }

    pub fn nothing(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.nothing
    }
    pub fn floating(&self, kind: crate::FloatKind) -> ImportedHirNominal<PersistentTypeId> {
        match kind {
            crate::FloatKind::F32 => self.float,
            crate::FloatKind::F64 => self.double,
        }
    }

    pub fn character(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.character
    }

    pub fn unit(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.unit
    }

    pub fn integer(&self, kind: IntegerKind) -> ImportedHirNominal<PersistentTypeId> {
        match kind {
            crate::IntegerKind::SIGNED_8 => self.int8,
            crate::IntegerKind::SIGNED_16 => self.int16,
            crate::IntegerKind::SIGNED_32 => self.int32,
            crate::IntegerKind::SIGNED_64 => self.int64,
            crate::IntegerKind::UNSIGNED_8 => self.uint8,
            crate::IntegerKind::UNSIGNED_16 => self.uint16,
            crate::IntegerKind::UNSIGNED_32 => self.uint32,
            crate::IntegerKind::UNSIGNED_64 => self.uint64,
        }
    }

    pub fn boolean(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.boolean
    }

    pub fn string(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.string
    }

    pub fn array(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.array
    }

    pub fn mutable_array(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.mutable_array
    }

    pub fn ptr(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.ptr
    }

    pub fn fun_ptr(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.fun_ptr
    }
}

impl ImportedCoreOptionProtocol {
    pub fn option(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.option
    }

    pub fn some(&self) -> ImportedHirId<PersistentEnumVariantId> {
        self.some
    }

    pub fn some_payload(&self) -> ImportedHirId<PersistentEnumVariantFieldId> {
        self.some_payload
    }

    pub fn none(&self) -> ImportedHirId<PersistentEnumVariantId> {
        self.none
    }
}

impl ImportedCoreIterationProtocol {
    pub fn iterator(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.iterator
    }

    pub fn next(&self) -> &ImportedCoreProtocolCallable {
        &self.next
    }

    pub fn next_dispatch(&self) -> ImportedHirId<PersistentDispatchSlotId> {
        self.next_dispatch
    }
}

impl ImportedCoreExceptionProtocol {
    pub fn missing_context_exception(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.missing_context_exception
    }

    pub fn missing_context_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        &self.missing_context_exception_constructor
    }
    pub fn throwable(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.throwable
    }

    pub fn throwable_constructor(&self) -> &ImportedCoreProtocolCallable {
        &self.throwable_constructor
    }

    pub fn unwrap_exception(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.unwrap_exception
    }

    pub fn unwrap_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        &self.unwrap_exception_constructor
    }

    pub fn class_cast_exception(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.class_cast_exception
    }

    pub fn class_cast_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        &self.class_cast_exception_constructor
    }

    pub fn arithmetic_exception(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.arithmetic_exception
    }

    pub fn arithmetic_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        &self.arithmetic_exception_constructor
    }

    pub fn index_out_of_bounds_exception(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.index_out_of_bounds_exception
    }

    pub fn index_out_of_bounds_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        &self.index_out_of_bounds_exception_constructor
    }

    pub fn illegal_state_exception(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.illegal_state_exception
    }

    pub fn illegal_state_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        &self.illegal_state_exception_constructor
    }

    pub fn illegal_argument_exception(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.illegal_argument_exception
    }

    pub fn illegal_argument_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        &self.illegal_argument_exception_constructor
    }

    pub fn initialization_cycle_thrower(&self) -> &ImportedCoreProtocolCallable {
        &self.initialization_cycle_thrower
    }
}

impl ImportedCoreCoroutineProtocol {
    pub fn continuation(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.continuation
    }

    pub fn continuation_resume(&self) -> &ImportedCoreProtocolCallable {
        &self.continuation_resume
    }

    pub fn continuation_resume_dispatch(&self) -> ImportedHirId<PersistentDispatchSlotId> {
        self.continuation_resume_dispatch
    }

    pub fn continuation_resume_with_exception(&self) -> &ImportedCoreProtocolCallable {
        &self.continuation_resume_with_exception
    }

    pub fn continuation_resume_with_exception_dispatch(
        &self,
    ) -> ImportedHirId<PersistentDispatchSlotId> {
        self.continuation_resume_with_exception_dispatch
    }

    pub fn suspend_task(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.suspend_task
    }

    pub fn suspend_task_run(&self) -> &ImportedCoreProtocolCallable {
        &self.suspend_task_run
    }

    pub fn suspend_task_run_dispatch(&self) -> ImportedHirId<PersistentDispatchSlotId> {
        self.suspend_task_run_dispatch
    }

    pub fn suspend_registration(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.suspend_registration
    }

    pub fn suspend_registration_register(&self) -> &ImportedCoreProtocolCallable {
        &self.suspend_registration_register
    }

    pub fn suspend_registration_register_dispatch(
        &self,
    ) -> ImportedHirId<PersistentDispatchSlotId> {
        self.suspend_registration_register_dispatch
    }

    pub fn start_coroutine(&self) -> &ImportedCoreProtocolCallable {
        &self.start_coroutine
    }

    pub fn suspend_coroutine(&self) -> &ImportedCoreProtocolCallable {
        &self.suspend_coroutine
    }
}

impl ImportedCoreFfiProtocol {
    pub fn ptr(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.ptr
    }

    pub fn fun_ptr(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.fun_ptr
    }

    pub fn pinned_ptr(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.pinned_ptr
    }

    pub fn gc_handle(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.gc_handle
    }

    pub fn ptr_to_ulong(&self) -> &ImportedCoreProtocolCallable {
        &self.ptr_to_ulong
    }

    pub fn ptr_cast(&self) -> &ImportedCoreProtocolCallable {
        &self.ptr_cast
    }

    pub fn ptr_load(&self) -> &ImportedCoreProtocolCallable {
        &self.ptr_load
    }

    pub fn ptr_load_offset(&self) -> &ImportedCoreProtocolCallable {
        &self.ptr_load_offset
    }

    pub fn ptr_store(&self) -> &ImportedCoreProtocolCallable {
        &self.ptr_store
    }

    pub fn ptr_store_offset(&self) -> &ImportedCoreProtocolCallable {
        &self.ptr_store_offset
    }

    pub fn ptr_plus(&self) -> &ImportedCoreProtocolCallable {
        &self.ptr_plus
    }

    pub fn ptr_minus(&self) -> &ImportedCoreProtocolCallable {
        &self.ptr_minus
    }

    pub fn address_of(&self) -> &ImportedCoreProtocolCallable {
        &self.address_of
    }

    pub fn size_of(&self) -> &ImportedCoreProtocolCallable {
        &self.size_of
    }

    pub fn align_of(&self) -> &ImportedCoreProtocolCallable {
        &self.align_of
    }

    pub fn gc_pin_raw(&self) -> &ImportedCoreProtocolCallable {
        &self.gc_pin_raw
    }

    pub fn gc_unpin_raw(&self) -> &ImportedCoreProtocolCallable {
        &self.gc_unpin_raw
    }

    pub fn gc_get_handle_raw(&self) -> &ImportedCoreProtocolCallable {
        &self.gc_get_handle_raw
    }

    pub fn gc_release_handle_raw(&self) -> &ImportedCoreProtocolCallable {
        &self.gc_release_handle_raw
    }
}

impl ImportedCoreForeignCallbackProtocol {
    pub fn callback(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        self.callback
    }

    pub fn mode(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.mode
    }

    pub fn reusable(&self) -> ImportedHirId<PersistentEnumVariantId> {
        self.reusable
    }

    pub fn one_shot(&self) -> ImportedHirId<PersistentEnumVariantId> {
        self.one_shot
    }

    pub fn state(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.state
    }

    pub fn registered(&self) -> ImportedHirId<PersistentEnumVariantId> {
        self.registered
    }

    pub fn active(&self) -> ImportedHirId<PersistentEnumVariantId> {
        self.active
    }

    pub fn completed(&self) -> ImportedHirId<PersistentEnumVariantId> {
        self.completed
    }

    pub fn failed(&self) -> ImportedHirId<PersistentEnumVariantId> {
        self.failed
    }

    pub fn failure_result(&self) -> ImportedHirId<PersistentExactTypeId> {
        self.failure_result
    }

    pub fn register(&self) -> &ImportedCoreProtocolCallable {
        &self.register
    }

    pub fn retain(&self) -> &ImportedCoreProtocolCallable {
        &self.retain
    }

    pub fn release(&self) -> &ImportedCoreProtocolCallable {
        &self.release
    }

    pub fn query_state(&self) -> &ImportedCoreProtocolCallable {
        &self.query_state
    }

    pub fn failure(&self) -> &ImportedCoreProtocolCallable {
        &self.failure
    }
}

impl ImportedCoreSourceLocationProtocol {
    pub fn location(&self) -> ImportedHirNominal<PersistentTypeId> {
        self.location
    }

    pub fn current(&self) -> &ImportedCoreProtocolCallable {
        &self.current
    }
}
