//! Typed access to the frontend language roles.

use super::*;

impl ImportedCoreFundamentalTypeProtocol {
    pub fn character(&self) -> ImportedHirNominal<PersistentTypeId> {
        concrete_nominal(&self.0, 15)
    }

    pub fn unit(&self) -> ImportedHirNominal<PersistentTypeId> {
        concrete_nominal(&self.0, 0)
    }

    pub fn integer(&self, kind: IntegerKind) -> ImportedHirNominal<PersistentTypeId> {
        let index = IntegerKind::ALL
            .iter()
            .position(|candidate| *candidate == kind)
            .expect("the closed integer kind belongs to IntegerKind::ALL");
        concrete_nominal(&self.0, index + 1)
    }

    pub fn boolean(&self) -> ImportedHirNominal<PersistentTypeId> {
        concrete_nominal(&self.0, 9)
    }

    pub fn string(&self) -> ImportedHirNominal<PersistentTypeId> {
        concrete_nominal(&self.0, 10)
    }

    pub fn array(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        generic_nominal(&self.0, 11)
    }

    pub fn mutable_array(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        generic_nominal(&self.0, 12)
    }

    pub fn ptr(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        generic_nominal(&self.0, 13)
    }

    pub fn fun_ptr(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        generic_nominal(&self.0, 14)
    }
}

impl ImportedCoreOptionProtocol {
    pub fn option(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
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
    pub fn iterator(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
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
    pub fn throwable(&self) -> ImportedHirNominal<PersistentTypeId> {
        concrete_nominal(&self.0, 0)
    }

    pub fn throwable_constructor(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 1)
    }

    pub fn unwrap_exception(&self) -> ImportedHirNominal<PersistentTypeId> {
        concrete_nominal(&self.0, 2)
    }

    pub fn unwrap_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 3)
    }

    pub fn class_cast_exception(&self) -> ImportedHirNominal<PersistentTypeId> {
        concrete_nominal(&self.0, 4)
    }

    pub fn class_cast_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 5)
    }

    pub fn arithmetic_exception(&self) -> ImportedHirNominal<PersistentTypeId> {
        concrete_nominal(&self.0, 6)
    }

    pub fn arithmetic_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 7)
    }

    pub fn index_out_of_bounds_exception(&self) -> ImportedHirNominal<PersistentTypeId> {
        concrete_nominal(&self.0, 8)
    }

    pub fn index_out_of_bounds_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 9)
    }

    pub fn illegal_state_exception(&self) -> ImportedHirNominal<PersistentTypeId> {
        concrete_nominal(&self.0, 10)
    }

    pub fn illegal_state_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 11)
    }

    pub fn illegal_argument_exception(&self) -> ImportedHirNominal<PersistentTypeId> {
        concrete_nominal(&self.0, 13)
    }

    pub fn illegal_argument_exception_constructor(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 14)
    }

    pub fn initialization_cycle_thrower(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 12)
    }
}

impl ImportedCoreCoroutineProtocol {
    pub fn continuation(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
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

    pub fn suspend_task(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        generic_nominal(&self.0, 5)
    }

    pub fn suspend_task_run(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 6)
    }

    pub fn suspend_task_run_dispatch(&self) -> ImportedHirId<PersistentDispatchSlotId> {
        dispatch_slot(&self.0, 7)
    }

    pub fn suspend_registration(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
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
    pub fn ptr(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        generic_nominal(&self.0, 0)
    }

    pub fn fun_ptr(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        generic_nominal(&self.0, 1)
    }

    pub fn pinned_ptr(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        generic_nominal(&self.0, 2)
    }

    pub fn gc_handle(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
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
    pub fn callback(&self) -> ImportedHirNominal<PersistentGenericTypeId> {
        generic_nominal(&self.0, 0)
    }

    pub fn mode(&self) -> ImportedHirNominal<PersistentTypeId> {
        concrete_nominal(&self.0, 1)
    }

    pub fn reusable(&self) -> ImportedHirId<PersistentEnumVariantId> {
        enum_variant(&self.0, 2)
    }

    pub fn one_shot(&self) -> ImportedHirId<PersistentEnumVariantId> {
        enum_variant(&self.0, 3)
    }

    pub fn state(&self) -> ImportedHirNominal<PersistentTypeId> {
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
    pub fn location(&self) -> ImportedHirNominal<PersistentTypeId> {
        concrete_nominal(&self.0, 0)
    }

    pub fn current(&self) -> &ImportedCoreProtocolCallable {
        callable(&self.0, 1)
    }
}
