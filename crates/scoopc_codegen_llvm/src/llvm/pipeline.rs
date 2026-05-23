//! LLVM pass pipeline 配置与执行。
//!
//! 这层只负责“给已经生成好的 module 跑哪条 LLVM pipeline”，
//! 不承担 HIR reachability、module build 或 emit API 入口职责。

use inkwell::passes::PassBuilderOptions;

use crate::opt::OptLevel;

use super::LlvmEmitError;

pub(crate) fn run_pass_pipeline<'ctx>(
    module: &inkwell::module::Module<'ctx>,
    target_machine: &inkwell::targets::TargetMachine,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    // 说明：
    // - T1503b：从手工 stackmap probe 迁移到 statepoint 产出的 stackmaps；
    // - C2a：在 statepoint 重写前跑 SROA，把“聚合值里的 GC ref 字段”拆解为可追踪 SSA 值，
    //   避免需要在源码里手工提取字段 keepalive。
    // - T1602：按 opt-level 启用 LLVM 默认优化 pipeline；同时保证大多数优化发生在 statepoint 重写之前。
    // - `rewrite-statepoints-for-gc` 会把带 `gc "<strategy>"` 的函数内调用点重写为 statepoints，
    //   并产出 stackmap records（用于 runtime 枚举/更新 spill slots roots）。
    // - 注意：LLVM 18.1.8（Homebrew）下 `place-safepoints` pass 会在 `opt` 上稳定触发 SIGSEGV，
    //   因此当前阶段不应把它纳入默认管线；需要 safepoint placement 时再结合上游修复/替代方案接入。
    let passes = llvm_pass_pipeline_for_opt_level(opt_level);
    let options = PassBuilderOptions::create();
    options.set_verify_each(true);
    module
        .run_passes(passes.as_str(), target_machine, options)
        .map_err(|e| LlvmEmitError::RunPassesFailed {
            passes: passes.clone(),
            message: e.to_string(),
        })?;

    module
        .verify()
        .map_err(|e| LlvmEmitError::ModuleVerificationFailed {
            message: e.to_string(),
        })?;

    Ok(())
}

fn llvm_pass_pipeline_for_opt_level(opt_level: OptLevel) -> String {
    let default_pipeline = match opt_level {
        OptLevel::O0 => None,
        OptLevel::O1 => Some("default<O1>"),
        OptLevel::O2 => Some("default<O2>"),
        OptLevel::O3 => Some("default<O3>"),
        OptLevel::Os => Some("default<Os>"),
        OptLevel::Oz => Some("default<Oz>"),
    };

    let mut passes = String::new();
    if let Some(default_pipeline) = default_pipeline {
        passes.push_str(default_pipeline);
        passes.push(',');
    }

    // GC/statepoint 约束：大多数优化放在 rewrite 之前；rewrite 之后只跑轻量清理，避免在
    // `gc.statepoint/gc.relocate` 之后引入更多不确定性。
    // 注意：moving GC 的 roots 更新目前只支持“可写回的 spill slots”（栈槽），不支持寄存器 roots。
    // 在 LLVM 后端启用 mem2reg 后，某些 GC 指针可能会长时间停留在寄存器中，导致 compaction 后
    // root 未被更新，从而在 `SCOOP_GC_STRESS=1` 下出现 use-after-move/语义错乱（T1606c）。
    //
    // v0 策略：在 statepoint rewrite 之前只跑 SROA，不跑 mem2reg，尽量让 roots 落在可写回的栈槽。
    passes.push_str("function(sroa),rewrite-statepoints-for-gc");
    if opt_level != OptLevel::O0 {
        passes.push_str(",function(instcombine,simplifycfg)");
    }

    passes
}
