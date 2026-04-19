//! 可在 typecheck / HIR lowering 路径复用的 resumed-step effect summary。
//!
//! 说明：
//! - 这里直接复用 `state_machine_plan.rs` 中的纯分析实现，保证与 LLVM effect lowering 使用
//!   同一份 resumed-step 语义，而不是在 typecheck 中维护一套独立近似；
//! - `MainCodegen` 相关入口已在源文件内按 `feature = "llvm"` 做条件编译，因此无 LLVM feature
//!   时这里仍可安全复用 direct-step summary API。

mod shared {
    include!("llvm/codegen/effect/state_machine_plan.rs");
}

pub(crate) use shared::compute_escape_continuation_direct_step_effect_rows_for_handle_in_program;
