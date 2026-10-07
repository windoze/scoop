use super::*;

/// Field offsets plus total size and alignment of an aggregate with
/// the given field types: each field sits at the next offset aligned
/// to its own alignment, and the size is rounded up to the aggregate
/// alignment (natural layout, as for LLVM literal structs). Enum
/// shapes come from `enum_shape`, so the same code serves both the
/// representation-computation phase and completed modules.
pub(crate) fn aggregate_shape(
    context: &LoweringContext,
    module: &mir::Module,
    enum_shape: &dyn Fn(mir::EnumId) -> StorageResult<(u64, u64)>,
    fields: &[mir::Type],
) -> StorageResult<(Vec<u64>, u64, u64)> {
    let mut offsets = Vec::with_capacity(fields.len());
    let mut cursor = lir::StorageLayoutCursorV1::new(
        context.target_profile(),
        lir::StoragePlacementPolicyV1::Ordinary,
    )?;
    for field in fields {
        let (size, align) = size_align(context, module, enum_shape, field)?;
        let geometry = lir::StorageGeometryV1::new(context.target_profile(), size, align)?;
        offsets.push(cursor.push(geometry)?.offset());
    }
    let geometry = cursor.finish()?;
    Ok((offsets, geometry.size(), geometry.alignment().get()))
}

/// Exact layout of one named struct. Ordinary structs use natural field
/// alignment. `@CLayout(packed = N)` caps each field's access alignment at
/// `N`; `aligned = N` raises (but never lowers) the aggregate alignment.
pub(crate) fn struct_shape(
    context: &LoweringContext,
    module: &mir::Module,
    enum_shape: &dyn Fn(mir::EnumId) -> StorageResult<(u64, u64)>,
    definition: &mir::StructDef,
) -> StorageResult<(Vec<lir::FieldLayout>, u64, u64)> {
    let mir::StructRepresentation::Declared {
        c_layout, fields, ..
    } = &definition.representation
    else {
        return Ok(match definition.representation {
            mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Unit) => {
                (Vec::new(), 0, 1)
            }
            mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Integer(
                kind,
            )) => {
                let layout = context.integer_layout(integer_kind(kind));
                (Vec::new(), layout.size, layout.align)
            }
            mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Float(kind)) => {
                let layout = context.float_layout(kind);
                (Vec::new(), layout.size, layout.align)
            }
            mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Char) => {
                let layout = context.scalar_layout(lir::BackendScalarKind::I32);
                (Vec::new(), layout.size, layout.align)
            }
            mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Boolean) => {
                let layout = context.scalar_layout(lir::BackendScalarKind::I1);
                (Vec::new(), layout.size, layout.align)
            }
            mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Ptr {
                ..
            }) => {
                let layout = context.pointer_layout(lir::PointerKind::Raw);
                (Vec::new(), layout.size, layout.align)
            }
            mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::FunPtr {
                ..
            }) => {
                let layout = context.pointer_layout(lir::PointerKind::Code);
                (Vec::new(), layout.size, layout.align)
            }
            mir::StructRepresentation::Intrinsic(
                mir::IntrinsicTypeRepresentation::String
                | mir::IntrinsicTypeRepresentation::Any
                | mir::IntrinsicTypeRepresentation::Nothing
                | mir::IntrinsicTypeRepresentation::Array { .. }
                | mir::IntrinsicTypeRepresentation::MutableArray { .. },
            ) => {
                unreachable!("the registry fixes intrinsic declaration targets")
            }
            mir::StructRepresentation::Declared { .. } => unreachable!(),
        });
    };
    if c_layout.is_some() && fields.is_empty() {
        return Err(lir::StorageReplayError::EmptyCLayout.into());
    }
    let policy = c_layout.map_or(lir::StoragePlacementPolicyV1::Ordinary, |contract| {
        lir::StoragePlacementPolicyV1::CLayout(lower_c_layout(contract))
    });
    let mut cursor = lir::StorageLayoutCursorV1::new(context.target_profile(), policy)?;
    let mut layouts = Vec::with_capacity(fields.len());
    for field in fields {
        let (size, align) = size_align(context, module, enum_shape, &field.ty)?;
        let geometry = lir::StorageGeometryV1::new(context.target_profile(), size, align)?;
        let placement = cursor.push(geometry)?;
        layouts.push(lir::FieldLayout {
            offset: placement.offset(),
            access_align: placement.access_alignment().get(),
        });
    }
    let geometry = cursor.finish()?;
    Ok((layouts, geometry.size(), geometry.alignment().get()))
}

/// Size and alignment of a value of type `ty`. `String` and the M6
/// reference types are pointers (pointer-sized); aggregates recurse;
/// enums take their representation's shape.
pub(crate) fn size_align(
    context: &LoweringContext,
    module: &mir::Module,
    enum_shape: &dyn Fn(mir::EnumId) -> StorageResult<(u64, u64)>,
    ty: &mir::Type,
) -> StorageResult<(u64, u64)> {
    let (size, align) = match ty {
        mir::Type::Context(storage) => {
            let layout = context.pointer_layout(lir::PointerKind::Managed);
            (
                layout.size
                    * if storage.role == mir::ContextStorageRole::Mark {
                        2
                    } else {
                        1
                    },
                layout.align,
            )
        }
        mir::Type::Unit => (0, 1),
        mir::Type::Integer(kind) => {
            let layout = context.integer_layout(integer_kind(*kind));
            (layout.size, layout.align)
        }
        mir::Type::MachineScalar(_) => {
            let layout = context.machine_scalar_layout();
            (layout.size, layout.align)
        }
        mir::Type::Boolean => {
            let layout = context.scalar_layout(lir::BackendScalarKind::I1);
            (layout.size, layout.align)
        }
        mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Function(_)
        | mir::Type::Any => {
            let layout = context.pointer_layout(lir::PointerKind::Managed);
            (layout.size, layout.align)
        }
        mir::Type::Ptr(_) => {
            let layout = context.pointer_layout(lir::PointerKind::Raw);
            (layout.size, layout.align)
        }
        mir::Type::FunPtr(_) => {
            let layout = context.pointer_layout(lir::PointerKind::Code);
            (layout.size, layout.align)
        }
        mir::Type::Struct(id) => {
            let (_, size, align) = struct_shape(context, module, enum_shape, &module.structs[*id])?;
            (size, align)
        }
        mir::Type::Tuple(elements) => {
            let (_, size, align) = aggregate_shape(context, module, enum_shape, elements)?;
            (size, align)
        }
        mir::Type::Enum(id, _) => enum_shape(*id)?,
    };
    let geometry = lir::StorageGeometryV1::new(context.target_profile(), size, align)?;
    Ok((geometry.size(), geometry.alignment().get()))
}
