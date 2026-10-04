//! Frame fields and exact saved slots, constructed before publishing adapters.

use super::*;

pub(super) struct ConstructedFrame {
    pub(super) class: mir::ClassId,
    pub(super) frame: mir::CoroutineFrameId,
    pub(super) layout: FrameLayout,
    pub(super) slots: HashMap<mir::LocalId, FrameSlot>,
    pub(super) saved_values: HashMap<mir::LocalId, mir::CoroutineSavedValueId>,
    pub(super) failure_slot: FrameSlot,
    pub(super) failure_value: mir::CoroutineFailureValueId,
}

pub(super) fn construct(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    coroutine: mir::CoroutineFunctionId,
    locals: &Arena<mir::Local>,
    completion_ty: &mir::Type,
    saved: &mut [mir::LocalId],
) -> ConstructedFrame {
    let metadata = &lowerer.coroutines.functions[coroutine];
    let function_id = metadata.function;
    let source_materialization = metadata.source;
    let source_odr_group = metadata.source_odr_group;
    let source_name = lowerer.functions[function_id].name.clone();
    let throwable_ty = crate::coroutine_registry::throwable_type(module, &lowerer.class_map);
    lowerer
        .source_exact_types
        .get(&throwable_ty)
        .expect("local-concrete HIR contains the Throwable exact type");
    let saved_identity_by_local = saved
        .iter()
        .map(|local| {
            (
                *local,
                lowerer
                    .local_values
                    .get(function_id, *local)
                    .unwrap_or_else(|| {
                        panic!(
                            "coroutine-saved local `{}` has no persistent value identity",
                            locals[*local].name
                        )
                    })
                    .clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    let frame_identity = mir::CoroutineFrameIdentity::new(
        source_materialization,
        saved_identity_by_local.values().cloned().collect(),
        source_odr_group,
    )
    .expect("a coroutine frame has one persistent generated identity");
    saved.sort_by_key(|local| saved_identity_by_local[local].id());

    let task_storage = mir::ContextStorageType::new(
        crate::context::core_provider(module),
        mir::ContextStorageRole::Task,
    );
    let mut frame_fields = vec![
        mir::Field {
            name: "state".to_string(),
            ty: mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineFrameState),
        },
        mir::Field {
            name: "completion".to_string(),
            ty: completion_ty.clone(),
        },
        mir::Field {
            name: "task".to_string(),
            ty: mir::Type::Context(task_storage),
        },
    ];
    let mut frame_slots = HashMap::new();
    for local in saved.iter() {
        let value_ty = locals[*local].ty.clone();
        let (slot_id, slot_ty) = lowerer.coroutines.slot_for(
            &lowerer.source_exact_types,
            &value_ty,
            &lowerer.structs,
            &mut lowerer.enums,
            &mut lowerer.shell,
        );
        let field = frame_fields.len() as u32;
        frame_fields.push(mir::Field {
            name: format!("local${}", locals[*local].name),
            ty: slot_ty.clone(),
        });
        frame_slots.insert(
            *local,
            FrameSlot::new(
                field,
                slot_id,
                slot_ty.clone(),
                &lowerer.coroutines.slots[slot_id],
            ),
        );
    }
    let (failure_slot_id, failure_slot_ty) = lowerer.coroutines.slot_for(
        &lowerer.source_exact_types,
        &throwable_ty,
        &lowerer.structs,
        &mut lowerer.enums,
        &mut lowerer.shell,
    );
    let failure_slot = FrameSlot::new(
        frame_fields.len() as u32,
        failure_slot_id,
        failure_slot_ty.clone(),
        &lowerer.coroutines.slots[failure_slot_id],
    );
    frame_fields.push(mir::Field {
        name: "failure".to_string(),
        ty: failure_slot_ty,
    });
    let frame_name = format!("CoroutineFrame<{source_name}>");
    let frame_class = generated_class(lowerer, frame_name, frame_fields, Vec::new(), Vec::new());
    let state_field = mir::CoroutineFrameFieldRef::checked(&lowerer.classes, frame_class, 0)
        .expect("the generated coroutine frame has a state field");
    let completion_field = mir::CoroutineFrameFieldRef::checked(&lowerer.classes, frame_class, 1)
        .expect("the generated coroutine frame has a completion field");
    let task_field = mir::CoroutineFrameFieldRef::checked(&lowerer.classes, frame_class, 2)
        .expect("the generated coroutine frame has its owning task field");
    let frame_layout = FrameLayout {
        state: state_field,
        completion: completion_field,
        task: task_field,
        task_storage,
    };
    let mut saved_value_by_local = HashMap::new();
    let mut saved_values = Vec::with_capacity(saved.len());
    for local in saved.iter() {
        let slot = &frame_slots[local];
        let field = mir::CoroutineFrameFieldRef::checked(&lowerer.classes, frame_class, slot.field)
            .expect("each generated coroutine saved slot has a typed frame field");
        let metadata = mir::CoroutineSavedValue::checked(
            &lowerer.classes,
            &lowerer.coroutines.slots,
            field,
            slot.slot,
        )
        .expect("each generated coroutine saved slot has its exact value type");
        let value = lowerer.coroutines.saved_values.alloc(metadata);
        saved_value_by_local.insert(*local, value);
        saved_values.push(value);
    }
    let failure_field =
        mir::CoroutineFrameFieldRef::checked(&lowerer.classes, frame_class, failure_slot.field)
            .expect("the generated coroutine frame has a typed failure field");
    let mir::Type::Class(throwable_class) = &throwable_ty else {
        unreachable!("the coroutine failure slot carries the canonical Throwable class")
    };
    let throwable_class = *throwable_class;
    let failure_metadata = mir::CoroutineFailureValue::checked(
        &lowerer.classes,
        &lowerer.coroutines.slots,
        failure_field,
        failure_slot.slot,
        throwable_class,
    )
    .expect("the generated coroutine failure slot carries exact Throwable");
    let failure_value = lowerer.coroutines.failure_values.alloc(failure_metadata);
    let frame_metadata = mir::CoroutineFrame::checked(
        &lowerer.classes,
        &lowerer.coroutines.saved_values,
        &lowerer.coroutines.failure_values,
        frame_class,
        coroutine,
        state_field,
        completion_field,
        task_field,
        saved_values,
        failure_value,
        frame_identity,
    )
    .expect("the generated coroutine frame has disjoint typed field roles");
    let frame = lowerer.coroutines.frames.alloc(frame_metadata);

    ConstructedFrame {
        class: frame_class,
        frame,
        layout: frame_layout,
        slots: frame_slots,
        saved_values: saved_value_by_local,
        failure_slot,
        failure_value,
    }
}
