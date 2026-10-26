//! Keep address arithmetic tied to the current managed pointer until RS4GC.

use std::collections::HashMap;

use inkwell::llvm_sys::core::{LLVMGetGC, LLVMReplaceAllUsesWith};

use super::*;

const OPERATION: &str = "scoop.gc-address-bits";

pub(super) fn protect(
    module: &LlvmModule<'_>,
    space: ManagedAddressSpace,
) -> Result<(), CodegenError> {
    let context = module.get_context();
    let builder = context.create_builder();
    let mut helpers = HashMap::new();
    let conversions = module
        .get_functions()
        .filter(|function| {
            // SAFETY: every function belongs to this live LLVM module.
            !unsafe { LLVMGetGC(function.as_value_ref()) }.is_null()
        })
        .flat_map(|function| function.get_basic_blocks())
        .flat_map(|block| block.get_instructions())
        .filter(|instruction| instruction.get_opcode() == InstructionOpcode::PtrToInt)
        .collect::<Vec<_>>();
    for instruction in conversions {
        let pointer = instruction
            .get_operand(0)
            .expect("ptrtoint has an operand")
            .value()
            .expect("ptrtoint has a value operand")
            .into_pointer_value();
        if pointer.get_type().get_address_space() != space.inkwell() {
            continue;
        }
        let result = instruction.get_type().into_int_type();
        let helper = *helpers.entry(result.get_bit_width()).or_insert_with(|| {
            let helper = module.add_function(
                "scoop.gc.address.bits",
                result.fn_type(&[pointer.get_type().into()], false),
                None,
            );
            helper.add_attribute(
                AttributeLoc::Function,
                context.create_string_attribute(OPERATION, ""),
            );
            helper.add_attribute(
                AttributeLoc::Function,
                context.create_string_attribute("gc-leaf-function", ""),
            );
            helper.add_attribute(
                AttributeLoc::Function,
                context.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
            );
            // LLVM 22 MemoryEffects encodes ModRef (3) in InaccessibleMem (1),
            // with two bits per location. Keep ordering across relocating
            // calls without clobbering ordinary object or stack memory.
            helper.add_attribute(
                AttributeLoc::Function,
                context.create_enum_attribute(Attribute::get_named_enum_kind_id("memory"), 3 << 2),
            );
            helper
        });
        builder.position_before(&instruction);
        let value = builder
            .build_call(helper, &[pointer.into()], "gc.address.bits")
            .map_err(error)?
            .try_as_basic_value()
            .basic()
            .expect("address conversion returns an integer");
        // SAFETY: the temporary operation has the original integer result type.
        unsafe { LLVMReplaceAllUsesWith(instruction.as_value_ref(), value.as_value_ref()) };
        instruction.erase_from_basic_block();
    }
    Ok(())
}

pub(in crate::statepoint) fn source(instruction: InstructionValue<'_>) -> Option<PointerValue<'_>> {
    let call = CallSiteValue::try_from(instruction).ok()?;
    let callee = call.get_called_fn_value()?;
    callee.get_string_attribute(AttributeLoc::Function, OPERATION)?;
    Some(instruction.get_operand(0)?.value()?.into_pointer_value())
}

pub(super) fn lower(module: &LlvmModule<'_>) -> Result<(), CodegenError> {
    let context = module.get_context();
    let builder = context.create_builder();
    let helpers = module
        .get_functions()
        .filter(|function| {
            function
                .get_string_attribute(AttributeLoc::Function, OPERATION)
                .is_some()
        })
        .collect::<Vec<_>>();
    for function in module.get_functions() {
        for block in function.get_basic_blocks() {
            let operations = block
                .get_instructions()
                .filter_map(|instruction| source(instruction).map(|pointer| (instruction, pointer)))
                .collect::<Vec<_>>();
            for (instruction, pointer) in operations {
                builder.position_before(&instruction);
                let value = builder
                    .build_ptr_to_int(
                        pointer,
                        instruction.get_type().into_int_type(),
                        "gc.address.bits",
                    )
                    .map_err(error)?;
                // SAFETY: RS4GC has updated the operand; the integer type is unchanged.
                unsafe { LLVMReplaceAllUsesWith(instruction.as_value_ref(), value.as_value_ref()) };
                instruction.erase_from_basic_block();
            }
        }
    }
    for helper in helpers {
        // SAFETY: only the converted direct calls referenced these private operations.
        unsafe { helper.delete() };
    }
    Ok(())
}

fn error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("managed address conversion: {error}"))
}
