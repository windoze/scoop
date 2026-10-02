use la_arena::Arena;
use scoop_hir::concrete;

#[derive(Debug, Clone)]
pub(super) struct PendingForeignCallbackRegistration {
    pub(super) source: scoop_hir::ForeignCallbackRegistrationId,
    pub(super) arguments: Vec<concrete::TypeId>,
    pub(super) callback: concrete::StructId,
    pub(super) native_function_type: concrete::FunctionTypeId,
    pub(super) managed_function_type: concrete::FunctionTypeId,
    pub(super) context_index: u32,
    pub(super) mode: scoop_identity::CallbackMode,
}

pub(super) fn finish_foreign_callback_slots(
    slots: Vec<PendingForeignCallbackRegistration>,
    applications: Vec<concrete::PersistentCallbackApplicationId>,
) -> Arena<concrete::ForeignCallbackRegistration> {
    assert_eq!(slots.len(), applications.len());
    let mut arena = Arena::new();
    for (index, (pending, application)) in slots.into_iter().zip(applications).enumerate() {
        let id = arena.alloc(concrete::ForeignCallbackRegistration {
            application,
            callback: pending.callback,
            native_function_type: pending.native_function_type,
            managed_function_type: pending.managed_function_type,
            context_index: pending.context_index,
            mode: pending.mode,
        });
        assert_eq!(id.into_raw().into_u32() as usize, index);
    }
    arena
}
