use super::*;

pub(super) fn scalar_carrier(ty: &lir::LirType, enums: &lir::EnumDefs) -> Option<lir::AbiCarrier> {
    use lir::{AbiCarrier as C, FloatKind, LirType as T};
    Some(match ty {
        T::I1 | T::I8 => C::Integer(8),
        T::I16 => C::Integer(16),
        T::I32 => C::Integer(32),
        T::I64 | T::MachineScalar(_) => C::Integer(64),
        T::F32 => C::Float(FloatKind::F32),
        T::F64 => C::Float(FloatKind::F64),
        T::Ptr(kind) => C::Pointer(*kind),
        T::Enum(id) => match enums[*id].repr {
            lir::EnumRepr::Niche { kind, .. } => C::Pointer(match kind {
                lir::NullNicheKind::Managed => lir::PointerKind::Managed,
                lir::NullNicheKind::Raw => lir::PointerKind::Raw,
                lir::NullNicheKind::Code => lir::PointerKind::Code,
                lir::NullNicheKind::Interface => return None,
            }),
            _ => return None,
        },
        _ => return None,
    })
}

pub(crate) fn aggregate_layout(
    context: &LoweringContext,
    ty: &lir::LirType,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::AbiAggregateLayout> {
    let (size, alignment) = safepoints::lir_size_align(context, ty, structs, enums)?;
    let mut result = lir::AbiAggregateLayout {
        size,
        alignment,
        leaves: Vec::new(),
    };
    collect(context, ty, structs, enums, 0, &mut result.leaves)?;
    Ok(result)
}

fn collect(
    context: &LoweringContext,
    ty: &lir::LirType,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    offset: u64,
    leaves: &mut Vec<lir::AbiScalarLeaf>,
) -> StorageResult<()> {
    let (size, alignment) = safepoints::lir_size_align(context, ty, structs, enums)?;
    if size == 0 {
        return Ok(());
    }
    if let Some(carrier) = scalar_carrier(ty, enums) {
        leaves.push(lir::AbiScalarLeaf {
            offset,
            carrier,
            alignment,
        });
        return Ok(());
    }
    match ty {
        lir::LirType::Struct(id) => {
            let definition = &structs[*id];
            for index in 0..definition.field_count() {
                let ty = definition
                    .field_storage_type(index)
                    .expect("existing field has a type");
                let field = definition
                    .field_layout(index)
                    .expect("existing field has a layout");
                collect(context, &ty, structs, enums, offset + field.offset, leaves)?;
            }
        }
        lir::LirType::Aggregate(fields) => {
            let mut cursor = lir::StorageLayoutCursorV1::new(
                context.target_profile(),
                lir::StoragePlacementPolicyV1::Ordinary,
            )?;
            for field in fields {
                let (size, align) = safepoints::lir_size_align(context, field, structs, enums)?;
                let placement = cursor.push(lir::StorageGeometryV1::new(
                    context.target_profile(),
                    size,
                    align,
                )?)?;
                collect(
                    context,
                    field,
                    structs,
                    enums,
                    offset + placement.offset(),
                    leaves,
                )?;
            }
        }
        lir::LirType::Enum(_) => {
            // Tagged payload is shared storage, not a variant-specific HFA.
            for chunk in (0..size).step_by(8) {
                leaves.push(lir::AbiScalarLeaf {
                    offset: offset + chunk,
                    carrier: lir::AbiCarrier::Integer(((size - chunk).min(8) * 8) as u8),
                    alignment: alignment.min(8),
                });
            }
        }
        _ => {
            return Err(crate::StorageLoweringError::InvalidRepresentation(
                "aggregate ABI classification requires GC-free value storage",
            ));
        }
    }
    Ok(())
}
