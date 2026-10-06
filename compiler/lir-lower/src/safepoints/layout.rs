use super::*;

pub(crate) fn lir_size_align(
    context: &LoweringContext,
    ty: &lir::LirType,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<(u64, u64)> {
    let (size, alignment) = match ty {
        lir::LirType::Void => (0, 1),
        lir::LirType::F32
        | lir::LirType::F64
        | lir::LirType::I1
        | lir::LirType::I8
        | lir::LirType::I16
        | lir::LirType::I32
        | lir::LirType::I64 => {
            let kind = match ty {
                lir::LirType::I1 => lir::BackendScalarKind::I1,
                lir::LirType::I8 => lir::BackendScalarKind::I8,
                lir::LirType::I16 => lir::BackendScalarKind::I16,
                lir::LirType::I32 => lir::BackendScalarKind::I32,
                lir::LirType::F32 => lir::BackendScalarKind::F32,
                lir::LirType::F64 => lir::BackendScalarKind::F64,
                _ => lir::BackendScalarKind::I64,
            };
            let layout = context.scalar_layout(kind);
            (layout.size, layout.align)
        }
        lir::LirType::MachineScalar(_) => {
            let layout = context.machine_scalar_layout();
            (layout.size, layout.align)
        }
        lir::LirType::Ptr(kind) => {
            let layout = context.pointer_layout(*kind);
            (layout.size, layout.align)
        }
        lir::LirType::ExceptionRecord => {
            let layout = context.exception_record_layout();
            (layout.size, layout.align)
        }
        lir::LirType::Aggregate(fields) => {
            let (_, size, align) = lir_aggregate_shape(context, fields, structs, enums)?;
            (size, align)
        }
        lir::LirType::Struct(id) => {
            if arena_index(*id) >= structs.len() {
                return Err(StorageLoweringError::InvalidRepresentation(
                    "unknown struct layout",
                ));
            }
            (structs[*id].size, structs[*id].align)
        }
        lir::LirType::Enum(id) => {
            if arena_index(*id) >= enums.len() {
                return Err(StorageLoweringError::InvalidRepresentation(
                    "unknown enum layout",
                ));
            }
            repr_shape(context, &enums[*id].repr)
        }
    };
    let geometry = lir::StorageGeometryV1::new(context.target_profile(), size, alignment)?;
    Ok((geometry.size(), geometry.alignment().get()))
}

pub(super) fn lir_aggregate_shape(
    context: &LoweringContext,
    fields: &[lir::LirType],
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<(Vec<u64>, u64, u64)> {
    let target = context.target_profile();
    let mut cursor =
        lir::StorageLayoutCursorV1::new(target, lir::StoragePlacementPolicyV1::Ordinary)?;
    let mut offsets = Vec::with_capacity(fields.len());
    for field in fields {
        let (size, alignment) = lir_size_align(context, field, structs, enums)?;
        offsets.push(
            cursor
                .push(lir::StorageGeometryV1::new(target, size, alignment)?)?
                .offset(),
        );
    }
    let geometry = cursor.finish()?;
    Ok((offsets, geometry.size(), geometry.alignment().get()))
}
