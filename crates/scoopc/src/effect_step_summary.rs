//! 可在 typecheck / HIR lowering 路径复用的 resumed-step effect summary。
//!
//! 说明：
//! - 这里直接复用 shared `effect_state_machine_analysis.rs` 中的纯分析实现，保证与 LLVM effect lowering 使用
//!   同一份 resumed-step 语义，而不是在 typecheck 中维护一套独立近似；
//! - `MainCodegen` 相关入口已在源文件内按 `feature = "llvm"` 做条件编译，因此无 LLVM feature
//!   时这里仍可安全复用 direct-step summary API。

// 当前非 LLVM 路径只消费 shared analysis 里的少量 direct-step summary API，其余规划器/
// structural-signature/helper 会表现为 intentional dead code，因此把告警边界收口在这里。
#[allow(dead_code)]
mod shared {
    include!("effect_state_machine_analysis.rs");
}

#[allow(dead_code)]
pub(crate) fn compute_escape_continuation_direct_step_effect_rows_for_handle_in_program(
    types: &crate::ty::TypeStore,
    handle: &crate::hir::HandleExpr,
    object_inits: &crate::hir::ObjectInitIndex,
    top_level_immutable_values: &crate::hir::TopLevelImmutableValueIndex,
) -> std::collections::HashMap<crate::hir::SymbolId, crate::ty::EffectRow> {
    shared::compute_escape_continuation_direct_step_effect_rows_for_handle_in_program(
        types,
        handle,
        object_inits,
        top_level_immutable_values,
    )
}
