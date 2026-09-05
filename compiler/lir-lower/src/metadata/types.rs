use super::*;

/// The `lir::EnumDefId` of a MIR enum (the arenas are transposed 1:1).
pub(crate) fn enum_def_id(id: mir::EnumId) -> lir::EnumDefId {
    lir::EnumDefId::from_raw(id.into_raw())
}

pub(crate) fn struct_def_id(id: mir::StructId) -> lir::StructDefId {
    lir::StructDefId::from_raw(id.into_raw())
}

/// MIR and LIR intentionally define their own IR-local scalar domains.  This
/// exhaustive projection is the only place where their identities cross the
/// stage boundary.
pub(crate) const fn machine_scalar_kind(kind: mir::MachineScalarKind) -> lir::MachineScalarKind {
    match kind {
        mir::MachineScalarKind::EnumTag => lir::MachineScalarKind::EnumTag,
        mir::MachineScalarKind::InitializationOutcome => {
            lir::MachineScalarKind::InitializationOutcome
        }
        mir::MachineScalarKind::CoroutineFrameState => lir::MachineScalarKind::CoroutineFrameState,
        mir::MachineScalarKind::CoroutineAdapterState => {
            lir::MachineScalarKind::CoroutineAdapterState
        }
        mir::MachineScalarKind::ForeignCallbackStatus => {
            lir::MachineScalarKind::ForeignCallbackStatus
        }
        mir::MachineScalarKind::PointerElementOffset => {
            lir::MachineScalarKind::PointerElementOffset
        }
    }
}

pub(crate) const fn machine_scalar_value(
    value: mir::MachineScalarValue,
) -> lir::MachineScalarValue {
    match value {
        mir::MachineScalarValue::EnumTag(tag) => lir::MachineScalarValue::EnumTag(tag),
        mir::MachineScalarValue::InitializationOutcome(outcome) => {
            lir::MachineScalarValue::InitializationOutcome(match outcome {
                mir::InitializationOutcome::RunInitializer => {
                    lir::InitializationOutcome::RunInitializer
                }
                mir::InitializationOutcome::Ready => lir::InitializationOutcome::Ready,
                mir::InitializationOutcome::Failed => lir::InitializationOutcome::Failed,
                mir::InitializationOutcome::Cycle => lir::InitializationOutcome::Cycle,
            })
        }
        mir::MachineScalarValue::CoroutineFrameState(state) => {
            lir::MachineScalarValue::CoroutineFrameState(match state {
                mir::CoroutineFrameState::Initial => lir::CoroutineFrameState::Initial,
                mir::CoroutineFrameState::Running => lir::CoroutineFrameState::Running,
                mir::CoroutineFrameState::Completed => lir::CoroutineFrameState::Completed,
                mir::CoroutineFrameState::Suspended(site) => lir::CoroutineFrameState::Suspended(
                    lir::CoroutineSuspendStateId::new(site.get())
                        .expect("a MIR coroutine suspension state is nonzero"),
                ),
                mir::CoroutineFrameState::ResumeFailure(site) => {
                    lir::CoroutineFrameState::ResumeFailure(
                        lir::CoroutineSuspendStateId::new(site.get())
                            .expect("a MIR coroutine suspension state is nonzero"),
                    )
                }
            })
        }
        mir::MachineScalarValue::CoroutineAdapterState(state) => {
            lir::MachineScalarValue::CoroutineAdapterState(match state {
                mir::CoroutineAdapterState::Registering => lir::CoroutineAdapterState::Registering,
                mir::CoroutineAdapterState::Waiting => lir::CoroutineAdapterState::Waiting,
                mir::CoroutineAdapterState::CompletingSuccess => {
                    lir::CoroutineAdapterState::CompletingSuccess
                }
                mir::CoroutineAdapterState::CompletingFailure => {
                    lir::CoroutineAdapterState::CompletingFailure
                }
                mir::CoroutineAdapterState::LatchedSuccess => {
                    lir::CoroutineAdapterState::LatchedSuccess
                }
                mir::CoroutineAdapterState::LatchedFailure => {
                    lir::CoroutineAdapterState::LatchedFailure
                }
                mir::CoroutineAdapterState::Consumed => lir::CoroutineAdapterState::Consumed,
            })
        }
        mir::MachineScalarValue::ForeignCallbackStatus(status) => {
            lir::MachineScalarValue::ForeignCallbackStatus(match status {
                mir::ForeignCallbackStatus::Returned => lir::ForeignCallbackStatus::Returned,
                mir::ForeignCallbackStatus::Threw => lir::ForeignCallbackStatus::Threw,
            })
        }
        mir::MachineScalarValue::PointerElementOffset(offset) => {
            lir::MachineScalarValue::PointerElementOffset(offset)
        }
    }
}

pub(crate) fn lower_structs(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
) -> Arena<lir::StructDef> {
    let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
    let mut structs = Arena::new();
    for (_, definition) in module.structs.iter() {
        let (field_layouts, size, align) = struct_shape(module, &enum_shape, definition);
        let (fields, c_layout, interior_mutable) = match &definition.representation {
            mir::StructRepresentation::Declared {
                c_layout,
                interior_mutable,
                fields,
            } => (
                fields
                    .iter()
                    .zip(field_layouts)
                    .map(|(field, layout)| lir::StructField {
                        ty: lir_type(&field.ty),
                        layout,
                    })
                    .collect(),
                c_layout.map(|layout| lir::CLayout {
                    aligned: layout.aligned,
                    packed: layout.packed,
                }),
                *interior_mutable,
            ),
            mir::StructRepresentation::Intrinsic(_) => (Vec::new(), None, false),
        };
        structs.alloc(lir::StructDef {
            name: definition.name.clone(),
            fields,
            size,
            align,
            c_layout,
            interior_mutable,
        });
    }
    structs
}

/// Map a MIR type onto its LIR value type (DESIGN 2.4 / 3.4): Unit is
/// the empty aggregate, String and the M6 reference types pointers,
/// struct / tuple literal aggregates of their mapped fields, and
/// enums `LirType::Enum` — their representation lives in the
/// `EnumDef`, so the mapping is the identity on enum ids. Intrinsic arrays are
/// ordinary classes here and therefore managed pointers; their element layout
/// lives only in typed `ArrayType` metadata.
pub(crate) fn lir_type(ty: &mir::Type) -> lir::LirType {
    match ty {
        mir::Type::Unit => lir::LirType::Aggregate(Vec::new()),
        // UInt shares Int's machine word (M9, spec 11.2): the same
        // `i64` at LIR, so codegen needs no UInt-specific handling.
        mir::Type::Int | mir::Type::UInt => lir::LirType::I64,
        mir::Type::MachineScalar(kind) => lir::LirType::MachineScalar(machine_scalar_kind(*kind)),
        mir::Type::Boolean => lir::LirType::I1,
        mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Function(_)
        | mir::Type::Any => lir::LirType::Ptr(lir::PointerKind::Managed),
        mir::Type::Ptr(_) => lir::LirType::Ptr(lir::PointerKind::Raw),
        mir::Type::FunPtr(_) => lir::LirType::Ptr(lir::PointerKind::Code),
        mir::Type::Struct(id) => lir::LirType::Struct(struct_def_id(*id)),
        mir::Type::Tuple(elements) => {
            lir::LirType::Aggregate(elements.iter().map(lir_type).collect())
        }
        mir::Type::Enum(id, _) => lir::LirType::Enum(enum_def_id(*id)),
    }
}

pub(crate) fn uses_indirect_result(enums: &Arena<lir::EnumDef>, ty: &lir::LirType) -> bool {
    match ty {
        lir::LirType::Aggregate(_) | lir::LirType::Struct(_) | lir::LirType::ExceptionRecord => {
            true
        }
        lir::LirType::Enum(id) => matches!(enums[*id].repr, lir::EnumRepr::Tagged { .. }),
        lir::LirType::Void
        | lir::LirType::I1
        | lir::LirType::I64
        | lir::LirType::MachineScalar(_)
        | lir::LirType::Ptr(_) => false,
    }
}
