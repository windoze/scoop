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
    enum_shape: &dyn Fn(mir::EnumId) -> (u64, u64),
    fields: &[mir::Type],
) -> (Vec<u64>, u64, u64) {
    let mut offsets = Vec::with_capacity(fields.len());
    let mut size = 0u64;
    let mut align = 1u64;
    for field in fields {
        let (field_size, field_align) = size_align(context, module, enum_shape, field);
        let offset = size.next_multiple_of(field_align);
        offsets.push(offset);
        size = offset + field_size;
        align = align.max(field_align);
    }
    (offsets, size.next_multiple_of(align), align)
}

/// Exact layout of one named struct. Ordinary structs use natural field
/// alignment. `@CLayout(packed = N)` caps each field's access alignment at
/// `N`; `aligned = N` raises (but never lowers) the aggregate alignment.
pub(crate) fn struct_shape(
    context: &LoweringContext,
    module: &mir::Module,
    enum_shape: &dyn Fn(mir::EnumId) -> (u64, u64),
    definition: &mir::StructDef,
) -> (Vec<lir::FieldLayout>, u64, u64) {
    let mir::StructRepresentation::Declared {
        c_layout, fields, ..
    } = &definition.representation
    else {
        return match definition.representation {
            mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Integer(
                kind,
            )) => {
                let layout = context.integer_layout(integer_kind(kind));
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
                | mir::IntrinsicTypeRepresentation::Array { .. }
                | mir::IntrinsicTypeRepresentation::MutableArray { .. },
            ) => {
                unreachable!("the registry fixes intrinsic declaration targets")
            }
            mir::StructRepresentation::Declared { .. } => unreachable!(),
        };
    };
    let packed = c_layout
        .and_then(|layout| layout.packed.bytes())
        .map(u64::from)
        .unwrap_or(0);
    let explicit_align = c_layout
        .and_then(|layout| layout.aligned.bytes())
        .map(u64::from)
        .unwrap_or(0);
    let mut layouts = Vec::with_capacity(fields.len());
    let mut size = 0u64;
    let mut align = explicit_align.max(1);
    for field in fields {
        let (field_size, natural_align) = size_align(context, module, enum_shape, &field.ty);
        let access_align = if packed == 0 {
            natural_align
        } else {
            natural_align.min(packed)
        };
        let offset = size.next_multiple_of(access_align);
        layouts.push(lir::FieldLayout {
            offset,
            access_align,
        });
        size = offset + field_size;
        align = align.max(access_align);
    }
    (layouts, size.next_multiple_of(align), align)
}

/// Size and alignment of a value of type `ty`. `String` and the M6
/// reference types are pointers (pointer-sized); aggregates recurse;
/// enums take their representation's shape.
pub(crate) fn size_align(
    context: &LoweringContext,
    module: &mir::Module,
    enum_shape: &dyn Fn(mir::EnumId) -> (u64, u64),
    ty: &mir::Type,
) -> (u64, u64) {
    match ty {
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
            let (_, size, align) = struct_shape(context, module, enum_shape, &module.structs[*id]);
            (size, align)
        }
        mir::Type::Tuple(elements) => {
            let (_, size, align) = aggregate_shape(context, module, enum_shape, elements);
            (size, align)
        }
        mir::Type::Enum(id, _) => enum_shape(*id),
    }
}
