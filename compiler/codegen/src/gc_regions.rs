//! Keep typed array mapping queries intact until bounded card loops are optimized.

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::builder::Builder;
use inkwell::module::{Linkage, Module};
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::TargetMachine;
use inkwell::values::{
    BasicValue, CallSiteValue, FunctionValue, InstructionValue, IntValue, PointerValue,
};
use inkwell::{AddressSpace, AtomicOrdering, IntPredicate};

use crate::CodegenError;

const OPERATION: &str = "scoop.gc.array-region";

pub(crate) fn is_array_query(instruction: InstructionValue<'_>) -> bool {
    CallSiteValue::try_from(instruction)
        .ok()
        .and_then(|call| call.get_called_fn_value())
        .is_some_and(|callee| {
            callee
                .get_string_attribute(AttributeLoc::Function, OPERATION)
                .is_some()
        })
}

pub(crate) fn array_mapping<'ctx>(
    module: &Module<'ctx>,
    builder: &Builder<'ctx>,
    address: IntValue<'ctx>,
) -> Result<PointerValue<'ctx>, CodegenError> {
    let context = module.get_context();
    let helper = module
        .get_functions()
        .find(|function| {
            function
                .get_string_attribute(AttributeLoc::Function, OPERATION)
                .is_some()
        })
        .unwrap_or_else(|| {
            let helper = module.add_function(
                "scoop.gc.array.region",
                context
                    .ptr_type(AddressSpace::default())
                    .fn_type(&[address.get_type().into()], false),
                None,
            );
            for name in [OPERATION, "gc-leaf-function"] {
                helper.add_attribute(
                    AttributeLoc::Function,
                    context.create_string_attribute(name, ""),
                );
            }
            helper.add_attribute(
                AttributeLoc::Function,
                context.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
            );
            // The query reads the GC size bits and reserved mapping prefix.
            // Its address operand retains the separate GC-address operation.
            helper.add_attribute(
                AttributeLoc::Function,
                context
                    .create_enum_attribute(Attribute::get_named_enum_kind_id("memory"), 0b01_01_01),
            );
            helper
        });
    Ok(builder
        .build_call(helper, &[address.into()], "array.mapping")
        .map_err(error)?
        .try_as_basic_value()
        .basic()
        .expect("the private mapping operation returns a pointer")
        .into_pointer_value())
}

pub(crate) fn page_mapping<'ctx>(
    module: &Module<'ctx>,
    builder: &Builder<'ctx>,
    address: IntValue<'ctx>,
) -> Result<PointerValue<'ctx>, CodegenError> {
    let context = module.get_context();
    let ptr = context.ptr_type(AddressSpace::default());
    let i64_ty = context.i64_type();
    let symbol = scoop_lir::RuntimeAbiSymbolV1::PageMap.logical_symbol();
    let root = module
        .get_global(symbol)
        .unwrap_or_else(|| module.add_global(ptr.array_type(4096), None, symbol));
    let mut node = root.as_pointer_value();
    for shift in [52, 40, 28, 16] {
        let shifted = builder
            .build_right_shift(address, i64_ty.const_int(shift, false), false, "page_shift")
            .map_err(error)?;
        let index = builder
            .build_and(shifted, i64_ty.const_int(4095, false), "page_index")
            .map_err(error)?;
        // SAFETY: a heap destination has a published path of 4096-slot nodes.
        let slot = unsafe { builder.build_gep(ptr, node, &[index], "page_slot") }.map_err(error)?;
        let loaded = builder.build_load(ptr, slot, "page_entry").map_err(error)?;
        loaded
            .as_instruction_value()
            .expect("page-map load is an instruction")
            .set_atomic_ordering(AtomicOrdering::Acquire)
            .map_err(|error| CodegenError(format!("page-map acquire load: {error}")))?;
        node = loaded.into_pointer_value();
    }
    Ok(node)
}

pub(crate) fn inline_array_queries(
    module: &Module<'_>,
    machine: &TargetMachine,
) -> Result<(), CodegenError> {
    let helpers = module
        .get_functions()
        .filter(|function| {
            function
                .get_string_attribute(AttributeLoc::Function, OPERATION)
                .is_some()
        })
        .collect::<Vec<_>>();
    if helpers.is_empty() {
        return Ok(());
    }
    for helper in helpers {
        define_query(module, helper)?;
    }
    module
        .run_passes("always-inline", machine, PassBuilderOptions::create())
        .map_err(|error| CodegenError(format!("array mapping inlining: {error}")))
}

fn define_query<'ctx>(
    module: &Module<'ctx>,
    helper: FunctionValue<'ctx>,
) -> Result<(), CodegenError> {
    let context = module.get_context();
    let builder = context.create_builder();
    let i64_ty = context.i64_type();
    let ptr = context.ptr_type(AddressSpace::default());
    helper.set_linkage(Linkage::Internal);
    helper.add_attribute(
        AttributeLoc::Function,
        context.create_enum_attribute(Attribute::get_named_enum_kind_id("alwaysinline"), 0),
    );
    let entry = context.append_basic_block(helper, "array.owner_region");
    builder.position_at_end(entry);
    let address = helper
        .get_nth_param(0)
        .expect("array query has its owner address")
        .into_int_value();
    let word_address = builder
        .build_int_add(address, i64_ty.const_int(8, false), "array.gc_address")
        .map_err(error)?;
    let word_slot = builder
        .build_int_to_ptr(word_address, ptr, "array.gc_slot")
        .map_err(error)?;
    let word = builder
        .build_load(i64_ty, word_slot, "array.gc_word")
        .map_err(error)?;
    word.as_instruction_value()
        .expect("GC word load is an instruction")
        .set_atomic_ordering(AtomicOrdering::Monotonic)
        .map_err(|error| CodegenError(format!("array GC word load: {error}")))?;
    let size = builder
        .build_and(
            word.into_int_value(),
            i64_ty.const_int(0x7ff8, false),
            "array.small_size",
        )
        .map_err(error)?;
    let small = builder
        .build_int_compare(
            IntPredicate::NE,
            size,
            i64_ty.const_zero(),
            "array.is_small",
        )
        .map_err(error)?;
    let small_base = builder
        .build_and(
            address,
            i64_ty.const_int(!16777215, false),
            "array.small_base",
        )
        .map_err(error)?;
    let large_base = builder
        .build_int_sub(address, i64_ty.const_int(128, false), "array.large_base")
        .map_err(error)?;
    // Select the valid address before loading. The other prefix can be
    // outside a mapping, and GC size bits can share a word with pin state.
    let base = builder
        .build_select(small, small_base, large_base, "array.mapping_base")
        .map_err(error)?;
    let slot = builder
        .build_int_to_ptr(base.into_int_value(), ptr, "array.region_slot")
        .map_err(error)?;
    let mapping = builder
        .build_load(ptr, slot, "array.owner_mapping")
        .map_err(error)?;
    builder.build_return(Some(&mapping)).map_err(error)?;
    Ok(())
}

fn error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("GC mapping query: {error}"))
}
