//! LLVM payloads for complete C-layout views.

use super::*;

pub(crate) fn c_field_size(
    structs: &StructDefs,
    enums: &EnumDefs,
    ty: &LirType,
) -> Result<u64, CodegenError> {
    Ok(match ty {
        LirType::I1 | LirType::I8 => 1,
        LirType::I16 => 2,
        LirType::I32 | LirType::F32 => 4,
        LirType::I64 | LirType::F64 | LirType::MachineScalar(_) | LirType::Ptr(_) => 8,
        LirType::Struct(id) => structs[*id].size,
        LirType::Enum(id) if matches!(enums[*id].repr, EnumRepr::Niche { .. }) => 8,
        other => {
            return Err(CodegenError(format!(
                "non-C field type {} reached a C-layout struct",
                other.dump()
            )));
        }
    })
}

pub(crate) fn c_payload_fields<'ctx>(
    context: &'ctx Context,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    layout: scoop_lir::CStructView<'_>,
) -> Result<Vec<BasicTypeEnum<'ctx>>, CodegenError> {
    let mut physical = Vec::new();
    let mut cursor = 0u64;
    let definition = layout.definition;
    let fields = layout.fields;
    for field in fields {
        let padding = field.layout.offset.checked_sub(cursor).ok_or_else(|| {
            CodegenError(format!(
                "overlapping fields in C layout `{}`",
                definition.name
            ))
        })?;
        if padding != 0 {
            physical.push(context.i8_type().array_type(padding as u32).into());
        }
        let storage_type = field.ty.storage_type();
        physical.push(basic_ty(
            context,
            structs,
            enums,
            managed_address_space,
            &storage_type,
        )?);
        cursor = field.layout.offset + c_field_size(structs, enums, &storage_type)?;
    }
    let tail = definition
        .size
        .checked_sub(cursor)
        .ok_or_else(|| CodegenError(format!("fields exceed C layout `{}`", definition.name)))?;
    if tail != 0 {
        physical.push(context.i8_type().array_type(tail as u32).into());
    }
    Ok(physical)
}

/// LLVM field index of a logical C-layout field inside the packed
/// payload struct. Explicit padding arrays occupy physical fields but
/// are deliberately absent from LIR's source-level field numbering.
pub(crate) fn c_physical_field_index(
    structs: &StructDefs,
    enums: &EnumDefs,
    layout: scoop_lir::CStructView<'_>,
    logical_index: u32,
) -> Result<u32, CodegenError> {
    let mut physical_index = 0u32;
    let mut cursor = 0u64;
    let definition = layout.definition;
    let fields = layout.fields;
    for (index, field) in fields.iter().enumerate() {
        if field.layout.offset > cursor {
            physical_index += 1;
        }
        if index == logical_index as usize {
            return Ok(physical_index);
        }
        physical_index += 1;
        cursor = field.layout.offset + c_field_size(structs, enums, &field.ty.storage_type())?;
    }
    Err(CodegenError(format!(
        "field {logical_index} out of range for C layout `{}`",
        definition.name
    )))
}
