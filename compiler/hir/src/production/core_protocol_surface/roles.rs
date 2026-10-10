//! Named roles of the serialized core compiler protocols.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreFundamentalTypeProtocolV1 {
    pub(crate) unit: PersistentTypeId,
    pub(crate) int8: PersistentTypeId,
    pub(crate) int16: PersistentTypeId,
    pub(crate) int32: PersistentTypeId,
    pub(crate) int64: PersistentTypeId,
    pub(crate) uint8: PersistentTypeId,
    pub(crate) uint16: PersistentTypeId,
    pub(crate) uint32: PersistentTypeId,
    pub(crate) uint64: PersistentTypeId,
    pub(crate) boolean: PersistentTypeId,
    pub(crate) string: PersistentTypeId,
    pub(crate) array: PersistentGenericTypeId,
    pub(crate) mutable_array: PersistentGenericTypeId,
    pub(crate) ptr: PersistentGenericTypeId,
    pub(crate) fun_ptr: PersistentGenericTypeId,
    pub(crate) character: PersistentTypeId,
    pub(crate) float: PersistentTypeId,
    pub(crate) double: PersistentTypeId,
    pub(crate) any: PersistentTypeId,
    pub(crate) nothing: PersistentTypeId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreOptionProtocolV1 {
    pub(crate) option: PersistentGenericTypeId,
    pub(crate) some: PersistentEnumVariantId,
    pub(crate) some_payload: PersistentEnumVariantFieldId,
    pub(crate) none: PersistentEnumVariantId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreIterationProtocolV1 {
    pub(crate) iterator: PersistentGenericTypeId,
    pub(crate) next: CoreProtocolCallableV1,
    pub(crate) next_dispatch: PersistentDispatchSlotId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreExceptionProtocolV1 {
    pub(crate) throwable: PersistentTypeId,
    pub(crate) throwable_constructor: CoreProtocolCallableV1,
    pub(crate) unwrap_exception: PersistentTypeId,
    pub(crate) unwrap_exception_constructor: CoreProtocolCallableV1,
    pub(crate) class_cast_exception: PersistentTypeId,
    pub(crate) class_cast_exception_constructor: CoreProtocolCallableV1,
    pub(crate) arithmetic_exception: PersistentTypeId,
    pub(crate) arithmetic_exception_constructor: CoreProtocolCallableV1,
    pub(crate) index_out_of_bounds_exception: PersistentTypeId,
    pub(crate) index_out_of_bounds_exception_constructor: CoreProtocolCallableV1,
    pub(crate) illegal_state_exception: PersistentTypeId,
    pub(crate) illegal_state_exception_constructor: CoreProtocolCallableV1,
    pub(crate) initialization_cycle_thrower: CoreProtocolCallableV1,
    pub(crate) illegal_argument_exception: PersistentTypeId,
    pub(crate) illegal_argument_exception_constructor: CoreProtocolCallableV1,
    pub(crate) missing_context_exception: PersistentTypeId,
    pub(crate) missing_context_exception_constructor: CoreProtocolCallableV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreCoroutineProtocolV1 {
    pub(crate) continuation: PersistentGenericTypeId,
    pub(crate) continuation_resume: CoreProtocolCallableV1,
    pub(crate) continuation_resume_dispatch: PersistentDispatchSlotId,
    pub(crate) continuation_resume_with_exception: CoreProtocolCallableV1,
    pub(crate) continuation_resume_with_exception_dispatch: PersistentDispatchSlotId,
    pub(crate) suspend_task: PersistentGenericTypeId,
    pub(crate) suspend_task_run: CoreProtocolCallableV1,
    pub(crate) suspend_task_run_dispatch: PersistentDispatchSlotId,
    pub(crate) suspend_registration: PersistentGenericTypeId,
    pub(crate) suspend_registration_register: CoreProtocolCallableV1,
    pub(crate) suspend_registration_register_dispatch: PersistentDispatchSlotId,
    pub(crate) start_coroutine: CoreProtocolCallableV1,
    pub(crate) suspend_coroutine: CoreProtocolCallableV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreFfiProtocolV1 {
    pub(crate) ptr: PersistentGenericTypeId,
    pub(crate) fun_ptr: PersistentGenericTypeId,
    pub(crate) pinned_ptr: PersistentGenericTypeId,
    pub(crate) gc_handle: PersistentGenericTypeId,
    pub(crate) ptr_to_ulong: CoreProtocolCallableV1,
    pub(crate) ptr_cast: CoreProtocolCallableV1,
    pub(crate) ptr_load: CoreProtocolCallableV1,
    pub(crate) ptr_load_offset: CoreProtocolCallableV1,
    pub(crate) ptr_store: CoreProtocolCallableV1,
    pub(crate) ptr_store_offset: CoreProtocolCallableV1,
    pub(crate) ptr_plus: CoreProtocolCallableV1,
    pub(crate) ptr_minus: CoreProtocolCallableV1,
    pub(crate) address_of: CoreProtocolCallableV1,
    pub(crate) size_of: CoreProtocolCallableV1,
    pub(crate) align_of: CoreProtocolCallableV1,
    pub(crate) gc_pin_raw: CoreProtocolCallableV1,
    pub(crate) gc_unpin_raw: CoreProtocolCallableV1,
    pub(crate) gc_get_handle_raw: CoreProtocolCallableV1,
    pub(crate) gc_release_handle_raw: CoreProtocolCallableV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreForeignCallbackProtocolV1 {
    pub(crate) callback: PersistentGenericTypeId,
    pub(crate) mode: PersistentTypeId,
    pub(crate) reusable: PersistentEnumVariantId,
    pub(crate) one_shot: PersistentEnumVariantId,
    pub(crate) state: PersistentTypeId,
    pub(crate) registered: PersistentEnumVariantId,
    pub(crate) active: PersistentEnumVariantId,
    pub(crate) completed: PersistentEnumVariantId,
    pub(crate) failed: PersistentEnumVariantId,
    pub(crate) failure_result: PersistentExactTypeId,
    pub(crate) register: CoreProtocolCallableV1,
    pub(crate) retain: CoreProtocolCallableV1,
    pub(crate) release: CoreProtocolCallableV1,
    pub(crate) query_state: CoreProtocolCallableV1,
    pub(crate) failure: CoreProtocolCallableV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreSourceLocationProtocolV1 {
    pub(crate) location: PersistentTypeId,
    pub(crate) current: CoreProtocolCallableV1,
}

impl CoreFundamentalTypeProtocolV1 {
    pub(crate) fn integer(&self, kind: crate::IntegerKind) -> PersistentTypeId {
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
}
