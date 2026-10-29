//! Normalize real aggregate reads without moving unrelated source calculations.

use inkwell::targets::TargetData;

use super::*;

pub(in crate::statepoint) fn normalize_loads(
    function: FunctionValue<'_>,
    layout: &TargetData,
    space: ManagedAddressSpace,
) -> Result<(), CodegenError> {
    let context = function.get_first_basic_block().unwrap().get_context();
    let builder = context.create_builder();
    let loads = function
        .get_basic_blocks()
        .into_iter()
        .flat_map(|block| block.get_instructions())
        .filter(|instruction| instruction.get_opcode() == InstructionOpcode::Load)
        .collect::<Vec<_>>();
    for load in loads {
        // SAFETY: a load always produces a basic LLVM value.
        let value = unsafe { BasicValueEnum::new(load.as_value_ref()) };
        let ty = value.get_type();
        if !matches!(
            ty,
            BasicTypeEnum::StructType(_) | BasicTypeEnum::ArrayType(_)
        ) || !contains_managed(ty, space)
            || load.get_volatile().unwrap()
        {
            continue;
        }
        builder.position_before(&load);
        if let Some(location) = load.get_debug_location() {
            builder.set_current_debug_location(location);
        } else {
            builder.unset_current_debug_location();
        }
        let pointer = operand(load, 0).into_pointer_value();
        let alignment = u64::from(load.get_alignment().unwrap());
        let mut rebuilt = ty.const_zero();
        for (path, field) in shape(ty, space) {
            let offset = field_offset(layout, ty, &path);
            let address = if offset == 0 {
                pointer
            } else {
                // SAFETY: the offset identifies a field of the original load.
                unsafe {
                    builder.build_gep(
                        context.i8_type(),
                        pointer,
                        &[context.i64_type().const_int(offset, false)],
                        "gc.field.address",
                    )
                }
                .map_err(|error| CodegenError(format!("aggregate field address: {error}")))?
            };
            let part = builder
                .build_load(field, address, "gc.field")
                .map_err(|error| CodegenError(format!("aggregate field load: {error}")))?;
            part.as_instruction_value()
                .unwrap()
                .set_alignment((1_u64 << (alignment | offset).trailing_zeros()) as u32)
                .map_err(|error| CodegenError(format!("aggregate field alignment: {error}")))?;
            rebuilt = insert(&builder, rebuilt, &path, part)?;
        }
        // SAFETY: rebuilding preserves the original aggregate type and all fields.
        unsafe {
            LLVMReplaceAllUsesWith(load.as_value_ref(), rebuilt.as_value_ref());
        }
        load.erase_from_basic_block();
    }
    Ok(())
}

fn field_offset(layout: &TargetData, mut ty: BasicTypeEnum<'_>, path: &[u32]) -> u64 {
    let mut offset = 0;
    for index in path {
        match ty {
            BasicTypeEnum::StructType(structure) => {
                offset += layout.offset_of_element(&structure, *index).unwrap();
                ty = structure.get_field_type_at_index(*index).unwrap();
            }
            BasicTypeEnum::ArrayType(array) => {
                ty = array.get_element_type();
                offset += u64::from(*index) * layout.get_abi_size(&ty);
            }
            _ => unreachable!("aggregate paths traverse structs and arrays"),
        }
    }
    offset
}
