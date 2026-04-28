//! Backend-agnostic effect state-machine planning skeleton.
//!
//! This module owns the shared plan -> segment -> unified contract pipeline. LLVM codegen consumes
//! the resulting `UnifiedHandleLoweringContract`; it does not own the middle-end analysis.

#![allow(dead_code)]

include!("analysis.rs");
include!("segments.rs");
include!("transform.rs");

/// Build the shared lowering contract for a `handle` expression.
///
/// Pipeline: `HandleExpr` -> plan -> segments (+ validation) -> unified state machine -> contract.
pub(crate) fn build_unified_lowering_contract(
    types: &TypeStore,
    handle: &hir::HandleExpr,
    context: &mut EffectAnalysisCtx,
) -> UnifiedHandleLoweringContract {
    context.extend_known_local_metadata_from_handle(handle);
    let source_plan = HandleStateMachinePlan::build_with_context(types, handle, context);

    let segment_list = source_plan.build_segment_list();
    #[cfg(debug_assertions)]
    if let Err(message) = segment_list.validate_builder_contract() {
        panic!("invalid handle segment builder contract: {message}");
    }

    #[cfg(debug_assertions)]
    {
        let segment_signature = segment_list.structural_signature();
        let rebuilt_plan = HandleStateMachinePlan::build_from_segments(&segment_list)
            .unwrap_or_else(|message| {
                panic!("failed to rebuild handle state machine plan: {message}")
            });
        let rebuilt_segment_list = rebuilt_plan.build_segment_list();
        let rebuilt_segment_signature = rebuilt_segment_list.structural_signature();
        if rebuilt_segment_signature != segment_signature {
            panic!(
                "segment round-trip mismatch: source={segment_signature} rebuilt={rebuilt_segment_signature}"
            );
        }
    }

    let machine = segment_list
        .build_unified_state_machine()
        .unwrap_or_else(|message| panic!("failed to build unified state machine: {message}"));

    UnifiedHandleLoweringContract { machine }
}
