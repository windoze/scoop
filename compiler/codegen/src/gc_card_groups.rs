//! Keep complete card-group records through loop optimization, then inline the guard.

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::llvm_sys::core::LLVMReplaceAllUsesWith;
use inkwell::module::{Linkage, Module};
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::TargetMachine;
use inkwell::values::{
    AsValueRef, BasicValue, CallSiteValue, FunctionValue, InstructionOpcode, InstructionValue,
    IntValue, PointerValue,
};
use inkwell::{AddressSpace, AtomicOrdering};

use crate::CodegenError;

#[derive(Clone, Copy)]
pub(crate) struct Record<'ctx> {
    pub instruction: InstructionValue<'ctx>,
    pub callee: FunctionValue<'ctx>,
    pub region: PointerValue<'ctx>,
    pub card: IntValue<'ctx>,
}

pub(crate) fn record(instruction: InstructionValue<'_>) -> Option<Record<'_>> {
    if instruction.get_opcode() != InstructionOpcode::Call || instruction.get_num_operands() != 3 {
        return None;
    }
    let callee = CallSiteValue::try_from(instruction)
        .ok()?
        .get_called_fn_value()?;
    if callee.get_name().to_bytes()
        != scoop_lir::RuntimeAbiSymbolV1::RecordDirtyCardGroup
            .logical_symbol()
            .as_bytes()
    {
        return None;
    }
    Some(Record {
        instruction,
        callee,
        region: instruction.get_operand(0)?.value()?.into_pointer_value(),
        card: instruction.get_operand(1)?.value()?.into_int_value(),
    })
}

pub(crate) fn inline_fast_paths(
    module: &Module<'_>,
    machine: &TargetMachine,
) -> Result<(), CodegenError> {
    let Some(runtime) =
        module.get_function(scoop_lir::RuntimeAbiSymbolV1::RecordDirtyCardGroup.logical_symbol())
    else {
        return Ok(());
    };
    if runtime.as_global_value().get_first_use().is_none() {
        return Ok(());
    }
    let context = module.get_context();
    let helper = module.add_function(
        "scoop.card_group.fast",
        runtime.get_type(),
        Some(Linkage::Internal),
    );
    for name in ["alwaysinline", "nounwind"] {
        helper.add_attribute(
            AttributeLoc::Function,
            context.create_enum_attribute(Attribute::get_named_enum_kind_id(name), 0),
        );
    }
    helper.add_attribute(
        AttributeLoc::Function,
        context.create_string_attribute("gc-leaf-function", ""),
    );
    runtime.add_attribute(
        AttributeLoc::Function,
        context.create_enum_attribute(Attribute::get_named_enum_kind_id("cold"), 0),
    );
    // SAFETY: both functions have the same exact type. Redirect existing calls
    // before building the helper's own call to the real runtime entry.
    unsafe {
        LLVMReplaceAllUsesWith(runtime.as_value_ref(), helper.as_value_ref());
    }

    let entry = context.append_basic_block(helper, "entry");
    let slow = context.append_basic_block(helper, "record");
    let done = context.append_basic_block(helper, "done");
    let builder = context.create_builder();
    builder.position_at_end(entry);
    let region = helper
        .get_nth_param(0)
        .expect("record has a region")
        .into_pointer_value();
    let card = helper
        .get_nth_param(1)
        .expect("record has a card index")
        .into_int_value();
    let ptr = context.ptr_type(AddressSpace::default());
    let i64 = context.i64_type();
    // SAFETY: ABI 17 publishes the immutable card-group pointer at offset 24.
    let slot = unsafe {
        builder.build_gep(
            context.i8_type(),
            region,
            &[i64.const_int(24, false)],
            "card.groups_slot",
        )
    }
    .map_err(error)?;
    let groups = builder
        .build_load(ptr, slot, "card.groups")
        .map_err(error)?
        .into_pointer_value();
    let index = builder
        .build_right_shift(card, i64.const_int(6, false), false, "card.group_index")
        .map_err(error)?;
    let group_type = context.struct_type(&[ptr.into(), ptr.into()], false);
    // SAFETY: one 16-byte node exists for each 64-card group of the mapping.
    let group =
        unsafe { builder.build_gep(group_type, groups, &[index], "card.group") }.map_err(error)?;
    let next = builder
        .build_load(ptr, group, "card.group_next")
        .map_err(error)?;
    next.as_instruction_value()
        .expect("group state is a load")
        .set_atomic_ordering(AtomicOrdering::Monotonic)
        .map_err(|error| CodegenError(format!("card group atomic load: {error}")))?;
    let absent = builder
        .build_is_null(next.into_pointer_value(), "card.group_absent")
        .map_err(error)?;
    builder
        .build_conditional_branch(absent, slow, done)
        .map_err(error)?;
    builder.position_at_end(slow);
    builder
        .build_call(runtime, &[region.into(), card.into()], "")
        .map_err(error)?;
    builder.build_unconditional_branch(done).map_err(error)?;
    builder.position_at_end(done);
    builder.build_return(None).map_err(error)?;
    module
        .run_passes("always-inline", machine, PassBuilderOptions::create())
        .map_err(|error| CodegenError(format!("card group fast-path inlining: {error}")))
}

fn error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("card group guard: {error}"))
}
