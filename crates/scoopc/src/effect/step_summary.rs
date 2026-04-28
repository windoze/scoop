//! 可在 typecheck / HIR lowering 路径复用的 resumed-step effect summary。
//!
//! 说明：
//! - 这里直接复用 shared state-machine analysis，保证与 LLVM effect lowering 使用
//!   同一份 resumed-step 语义，而不是在 typecheck 中维护一套独立近似；
//! - no-LLVM 路径只消费 direct-step summary API，shared 模块内部的其它 planning skeleton
//!   由模块自身收口 dead-code 告警边界。

#[allow(dead_code)]
pub(crate) fn compute_escape_continuation_direct_step_effect_rows_for_handle_in_program(
    types: &crate::ty::TypeStore,
    handle: &crate::hir::HandleExpr,
    object_inits: &crate::hir::ObjectInitIndex,
    top_level_immutable_values: &crate::hir::TopLevelImmutableValueIndex,
) -> std::collections::HashMap<crate::hir::SymbolId, crate::ty::EffectRow> {
    crate::effect::state_machine::compute_escape_continuation_direct_step_effect_rows_for_handle_in_program(
        types,
        handle,
        object_inits,
        top_level_immutable_values,
    )
}
