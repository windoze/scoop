//! Select actual bounded scalar kernels in Managed functions.

use inkwell::llvm_sys::core::{LLVMReplaceMDNodeOperandWith, LLVMValueAsMetadata};

use super::*;

mod card_regions;
mod operations;
mod region;
mod remarks;

pub(super) fn optimize(
    function: FunctionValue<'_>,
    machine: &TargetMachine,
) -> Result<(), CodegenError> {
    let context = function.get_type().get_context();
    let mut selected = false;
    for latch in function.get_basic_blocks() {
        let Some(mut region) = region::analyze(latch) else {
            continue;
        };
        card_regions::prepare(&mut region)?;
        if !operations::prepare(&region) {
            continue;
        }
        let enable = context.metadata_node(&[
            context.metadata_string("llvm.loop.vectorize.enable").into(),
            context.bool_type().const_int(1, false).into(),
        ]);
        let interleave = context.metadata_node(&[
            context.metadata_string("llvm.loop.interleave.count").into(),
            context.i32_type().const_int(1, false).into(),
        ]);
        let unroll =
            context.metadata_node(&[context.metadata_string("llvm.loop.unroll.disable").into()]);
        let node = context.metadata_node(&[
            enable.into(),
            enable.into(),
            interleave.into(),
            unroll.into(),
        ]);
        // SAFETY: the live node belongs to this context. LLVM loop metadata
        // requires its first operand to refer to the loop node itself.
        unsafe {
            LLVMReplaceMDNodeOperandWith(
                node.as_value_ref(),
                0,
                LLVMValueAsMetadata(node.as_value_ref()),
            );
        }
        region
            .latch
            .get_terminator()
            .expect("qualified loop has a branch")
            .set_metadata(node, context.get_kind_id("llvm.loop"))
            .map_err(|error| CodegenError(format!("bounded loop metadata: {error:?}")))?;
        selected = true;
    }
    if selected {
        let _remarks = remarks::Quiet::new(context);
        function.run_passes(
            "loop-simplify,lcssa,loop-vectorize<interleave-forced-only;vectorize-forced-only>,instcombine<no-verify-fixpoint>,adce",
            machine, PassBuilderOptions::create(),
        ).map_err(|error| CodegenError(format!("bounded inner loop optimization failed: {error}")))?;
    }
    Ok(())
}
