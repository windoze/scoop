use super::*;

/// The `lir::EnumDefId` of a MIR enum (the arenas are transposed 1:1).
pub(crate) fn enum_def_id(id: mir::EnumId) -> lir::EnumDefId {
    lir::EnumDefId::from_raw(id.into_raw())
}

/// Map one MIR-local checked variant identity into the independently checked
/// LIR identity owned by the completed enum-layout store.
pub(crate) fn variant_ref(
    enums: &lir::EnumDefs,
    variant: mir::MirVariantRef,
) -> lir::LirVariantRef {
    enums
        .variant_ref(enum_def_id(variant.enum_id()), variant.variant_index())
        .expect("a checked MIR variant maps to the same completed LIR enum definition")
}

/// Map one MIR-local checked payload field into the LIR layout authority.
/// Field type and pointer provenance are deliberately revalidated here rather
/// than reconstructed from the MIR result expression.
pub(crate) fn variant_field_ref(
    enums: &lir::EnumDefs,
    field: mir::MirVariantFieldRef,
) -> lir::LirVariantFieldRef {
    enums
        .variant_field_ref(variant_ref(enums, field.variant()), field.field_index())
        .expect("a checked MIR variant field exists in the completed LIR enum layout")
}

pub(crate) fn struct_def_id(id: mir::StructId) -> lir::StructDefId {
    lir::StructDefId::from_raw(id.into_raw())
}

/// MIR and LIR own distinct source-integer domains. These exhaustive
/// projections are the only way exact integer semantics cross this stage.
pub(crate) const fn integer_signedness(
    signedness: mir::IntegerSignedness,
) -> lir::IntegerSignedness {
    match signedness {
        mir::IntegerSignedness::Signed => lir::IntegerSignedness::Signed,
        mir::IntegerSignedness::Unsigned => lir::IntegerSignedness::Unsigned,
    }
}

pub(crate) const fn integer_width(width: mir::IntegerWidth) -> lir::IntegerWidth {
    match width {
        mir::IntegerWidth::W8 => lir::IntegerWidth::W8,
        mir::IntegerWidth::W16 => lir::IntegerWidth::W16,
        mir::IntegerWidth::W32 => lir::IntegerWidth::W32,
        mir::IntegerWidth::W64 => lir::IntegerWidth::W64,
    }
}

pub(crate) const fn integer_kind(kind: mir::IntegerKind) -> lir::IntegerKind {
    lir::IntegerKind::new(
        integer_signedness(kind.signedness()),
        integer_width(kind.width()),
    )
}

pub(crate) const fn integer_constant(value: mir::MirIntegerConstant) -> lir::LirIntegerConstant {
    match value {
        mir::MirIntegerConstant::Signed8(bits) => lir::LirIntegerConstant::Signed8(bits),
        mir::MirIntegerConstant::Signed16(bits) => lir::LirIntegerConstant::Signed16(bits),
        mir::MirIntegerConstant::Signed32(bits) => lir::LirIntegerConstant::Signed32(bits),
        mir::MirIntegerConstant::Signed64(bits) => lir::LirIntegerConstant::Signed64(bits),
        mir::MirIntegerConstant::Unsigned8(bits) => lir::LirIntegerConstant::Unsigned8(bits),
        mir::MirIntegerConstant::Unsigned16(bits) => lir::LirIntegerConstant::Unsigned16(bits),
        mir::MirIntegerConstant::Unsigned32(bits) => lir::LirIntegerConstant::Unsigned32(bits),
        mir::MirIntegerConstant::Unsigned64(bits) => lir::LirIntegerConstant::Unsigned64(bits),
    }
}

pub(crate) const fn c_layout_value(value: mir::MirCLayoutValue) -> lir::LirCLayoutValue {
    match value {
        mir::MirCLayoutValue::Natural => lir::LirCLayoutValue::Natural,
        mir::MirCLayoutValue::A1 => lir::LirCLayoutValue::A1,
        mir::MirCLayoutValue::A2 => lir::LirCLayoutValue::A2,
        mir::MirCLayoutValue::A4 => lir::LirCLayoutValue::A4,
        mir::MirCLayoutValue::A8 => lir::LirCLayoutValue::A8,
        mir::MirCLayoutValue::A16 => lir::LirCLayoutValue::A16,
    }
}

pub(crate) const fn lower_c_layout(contract: mir::MirCLayoutContract) -> lir::LirCLayoutContract {
    lir::LirCLayoutContract {
        aligned: c_layout_value(contract.aligned),
        packed: c_layout_value(contract.packed),
    }
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
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::StructDefs> {
    let enum_shape = |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
    let mut structs = lir::StructDefs::default();
    for (_, definition) in module.structs.iter() {
        let (_, size, align) = struct_shape(context, module, &enum_shape, definition)?;
        match &definition.representation {
            mir::StructRepresentation::Declared {
                c_layout,
                interior_mutable,
                ..
            } => {
                if let Some(contract) = c_layout {
                    let _ = structs.alloc_c(
                        definition.name.clone(),
                        size,
                        align,
                        *interior_mutable,
                        lower_c_layout(*contract),
                        Vec::new(),
                    );
                } else {
                    structs.alloc_scoop(
                        definition.name.clone(),
                        size,
                        align,
                        *interior_mutable,
                        Vec::new(),
                    );
                }
            }
            mir::StructRepresentation::Intrinsic(representation) => {
                structs.alloc_intrinsic(
                    definition.name.clone(),
                    size,
                    align,
                    lower_intrinsic_type_representation(module, representation),
                );
            }
        }
    }

    // Fill declared fields only after every nominal shell is present. Exact C
    // pointer pointees may refer to the containing or a later C struct, and a
    // checked `CStructRef` must be minted against the actual completed arena.
    for (mir_id, definition) in module.structs.iter() {
        let mir::StructRepresentation::Declared {
            c_layout, fields, ..
        } = &definition.representation
        else {
            continue;
        };
        let (field_layouts, _, _) = struct_shape(context, module, &enum_shape, definition)?;
        if c_layout.is_some() {
            let reference = structs
                .c_ref(struct_def_id(mir_id))
                .expect("the first pass allocated a C struct shell");
            structs.set_c_fields(
                reference,
                fields
                    .iter()
                    .zip(field_layouts)
                    .map(|(field, layout)| lir::CStructField {
                        identity: field.identity,
                        ty: c_ffi_type(module, &structs, enums, &field.ty),
                        layout,
                    })
                    .collect(),
            );
        } else {
            structs.set_scoop_fields(
                struct_def_id(mir_id),
                fields
                    .iter()
                    .zip(field_layouts)
                    .map(|(field, layout)| lir::StructField {
                        ty: lir_type(&field.ty),
                        layout,
                    })
                    .collect(),
            );
        }
    }
    Ok(structs)
}

pub(crate) fn compiler_data_pointee(pointee: &mir::Type) -> lir::LirDataPointee {
    if pointee == &mir::Type::Unit {
        lir::LirDataPointee::OpaqueVoid
    } else {
        lir::LirDataPointee::Value(Box::new(lir_type(pointee)))
    }
}

pub(crate) fn compiler_function_type(
    module: &mir::Module,
    signature: mir::FunctionTypeId,
) -> lir::LirFunctionType {
    let signature = &module.function_types[signature];
    lir::LirFunctionType {
        params: signature.parameter_types.iter().map(lir_type).collect(),
        return_type: lir_return_type(&signature.return_type),
    }
}

pub(crate) fn lir_return_type(ty: &mir::Type) -> lir::LirReturnType {
    if ty == &mir::Type::Unit {
        lir::LirReturnType::Void
    } else {
        lir::LirReturnType::Value(Box::new(lir_type(ty)))
    }
}

pub(crate) fn lower_intrinsic_type_representation(
    module: &mir::Module,
    representation: &mir::IntrinsicTypeRepresentation,
) -> lir::IntrinsicTypeRepresentation {
    match representation {
        mir::IntrinsicTypeRepresentation::Integer(kind) => {
            lir::IntrinsicTypeRepresentation::Integer(integer_kind(*kind))
        }
        mir::IntrinsicTypeRepresentation::Boolean => lir::IntrinsicTypeRepresentation::Boolean,
        mir::IntrinsicTypeRepresentation::String => lir::IntrinsicTypeRepresentation::String,
        mir::IntrinsicTypeRepresentation::Ptr { pointee } => {
            lir::IntrinsicTypeRepresentation::Ptr {
                pointee: compiler_data_pointee(pointee),
            }
        }
        mir::IntrinsicTypeRepresentation::FunPtr { signature } => {
            lir::IntrinsicTypeRepresentation::FunPtr {
                signature: compiler_function_type(module, *signature),
            }
        }
        mir::IntrinsicTypeRepresentation::Array { .. }
        | mir::IntrinsicTypeRepresentation::MutableArray { .. } => {
            unreachable!("intrinsic arrays use the typed ArrayType metadata arena")
        }
    }
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
        mir::Type::Integer(kind) => integer_kind(*kind).scalar_type(),
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
