//! Optimize actual bounded kernels in Managed functions.

use inkwell::llvm_sys::core::{LLVMReplaceMDNodeOperandWith, LLVMValueAsMetadata};

use super::*;

mod card_regions;
mod edges;
mod operations;
mod reference_reads;
mod region;
mod remarks;

pub(super) fn optimize(
    function: FunctionValue<'_>,
    machine: &TargetMachine,
) -> Result<(), CodegenError> {
    let context = function.get_type().get_context();
    let mut vectorize = false;
    let mut unroll = false;
    for latch in function.get_basic_blocks() {
        let Some(mut region) = region::analyze(latch) else {
            continue;
        };
        card_regions::prepare(&mut region, machine)?;
        let scalar = operations::prepare(&region);
        if !scalar && !reference_reads::selected(&region) {
            continue;
        }
        if !scalar {
            // One branch can be the latch of both inner and outer loops.
            // Give the outer backedge its own terminator before adding a
            // count hint, so LoopSimplify cannot copy it to the outer loop.
            edges::split(region.latch, region.exit, "read.exit")?;
        }
        let enable = context.metadata_node(&[
            context.metadata_string("llvm.loop.vectorize.enable").into(),
            context
                .bool_type()
                .const_int(u64::from(scalar), false)
                .into(),
        ]);
        let interleave = context.metadata_node(&[
            context.metadata_string("llvm.loop.interleave.count").into(),
            context.i32_type().const_int(1, false).into(),
        ]);
        let unroll_hint = if scalar {
            context.metadata_node(&[context.metadata_string("llvm.loop.unroll.disable").into()])
        } else {
            context.metadata_node(&[
                context.metadata_string("llvm.loop.unroll.count").into(),
                context
                    .i32_type()
                    .const_int(reference_reads::FACTOR, false)
                    .into(),
            ])
        };
        let node = context.metadata_node(&[
            enable.into(),
            enable.into(),
            interleave.into(),
            unroll_hint.into(),
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
        vectorize |= scalar;
        unroll |= !scalar;
    }
    if vectorize {
        let _remarks = remarks::Quiet::new(context);
        function.run_passes(
            "loop-simplify,lcssa,loop-vectorize<interleave-forced-only;vectorize-forced-only>,instcombine<no-verify-fixpoint>,adce",
            machine, PassBuilderOptions::create(),
        ).map_err(|error| CodegenError(format!("bounded inner loop optimization failed: {error}")))?;
    }
    if unroll {
        let _remarks = remarks::Quiet::new(context);
        // Explicit counts remain effective; automatic full/partial unrolling
        // and peeling must not duplicate outer statepoints or root restores.
        function.run_passes(
            "loop-simplify,lcssa,loop-unroll<O2;full-unroll-max=0;no-partial;no-peeling;no-profile-peeling;no-runtime;no-upperbound>,instcombine<no-verify-fixpoint>,adce",
            machine, PassBuilderOptions::create(),
        ).map_err(|error| CodegenError(format!("bounded reference loop optimization failed: {error}")))?;
    }
    Ok(())
}
