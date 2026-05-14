# 执行计划

说明：我不会记录逐字的内部推理过程，但会持续在这里维护可检查的执行计划、关键决策、进度与阻塞信息。

## 当前回合目标

完成 `TODO.md` 中按顺序出现的第一个未完成任务；若遇到阻塞，则按要求在 `TODO.md` 中插入最小必要前置任务并停止。

## 初始步骤

1. 读取 `TODO.md`，识别第一个标题中未带 `[DONE]` 的任务。
2. 读取与该任务直接相关的上下文文件（必要时包括 `PLAN.md`、相关源码、测试、最新提交信息），确认范围、依赖与验证要求。
3. 如发现最新提交里存在与该任务直接相关且未完成的问题，将其视为当前任务的一部分；若它构成真实前置阻塞，则先在 `TODO.md` 中登记该前置任务。
4. 实现当前任务，避免规避性方案；若暴露出阻塞当前任务的规范不匹配或实现缺口，则先修复，或把它登记为前置任务并停止。
5. 运行与当前任务直接相关的验证；在可行范围内补充必要测试，并执行任务要求的检查。
6. 更新 `TODO.md`：仅在任务真正完成时给任务标题加上 `[DONE]`，并填写/更新完成记录；若发生任务拆分或重排，则同步维护顺序与依赖。
7. 仅在阶段计划发生变化时更新 `PLAN.md`。
8. 提交当前变更，提交信息以当前任务 ID 为前缀，然后停止，不继续下一个任务。

## 进度记录

- 已创建本计划文件，准备开始读取 `TODO.md` 并确定当前任务。
- 已确认当前回合的第一个未完成任务是 `P6-T01：收尾 §3.5 / §7.6 partial surface，统一 runtime cast 与 GC pin/handle policy`。
- 下一步：读取 `PLAN.md` / `PIPELINE_GAPS.md` 的对应条目，并检查最新提交是否记录了与 `P6-T01` 直接相关的未完成问题；随后审视当前实现与测试基线，决定直接实现还是先登记前置阻塞。
- 已确认最新提交没有单独声明一个会阻塞 `P6-T01` 的未完成问题；当前工作可直接在 `P6-T01` 内收口。
- 已完成首轮实现：
  - 把 GC pin/handle 的 typecheck 诊断与相关注释改为稳定 contract 语言，移除“当前阶段/临时 special-case”表述。
  - 新增 `GC.unpin` / `GC.handleGet` / `GC.handleDrop` 的错误 fixture，明确剩余未开放 surface 必须前端拒绝。
  - 将 runtime GC callback/raw token fixtures 中的 `GcHandle.raw` 类型统一到 `UIntPtr`，与 sysroot 与 typecheck fixture 对齐。
  - 将 `PIPELINE_GAPS.md` 与 `codegen_gap_inventory.rs` / `pipeline_gap_audit.rs` 中 `§3.5`、`§7.6` 的状态准备收口到 guard-only 语义。
- 在验证中暴露并已修复两个直接阻塞 `P6-T01` 的实现缺口：
  - 真实 HIR stage 路径不会为保留 member-access 形状的 GC intrinsic call 发布 typed call-site contract；现已在 HIR stage 补齐 intrinsic 合同，并在 typecheck 侧固定实参绑定回写。
  - typed HIR/MIR 把 `UIntPtr` 当作 ref nominal，而不是 word-sized scalar；现已在 HIR type lowering 中把 `UIntPtr` 与 `UInt` 对齐，并新增 MIR 回归锁定 `GcHandle.raw` token transport 为 scalar。
- 已完成验证：
  - `cargo test -p scoopc refactor_hir_gc_intrinsic_member_calls_publish_intrinsic_contracts`
  - `cargo test -p scoopc refactor_mir_value_primitives`
  - `cargo test -p scoopc refactor_mir_gc_handle_raw_uintptr_token_stays_scalar`
  - `cargo test -p scoopc refactor_llvm_runtime_type_primitives`
  - `cargo test -p scoopc codegen_gap_inventory`
  - `cargo test -p scoopc pipeline_gap_audit`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/runtime_typecheck_cast.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_{closed_pure_asq,effectful_asq,effectful_as}_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/{unsafe_nogc/gc_pin_value_type_is_error,typecheck/gc_handle_new_value_type_is_error,typecheck/gc_unpin_requires_pinned_is_error,typecheck/gc_handle_get_requires_handle_is_error,typecheck/gc_handle_drop_requires_handle_is_error,typecheck/extern_fun_gc_handle_raw_token_roundtrip_ok,typecheck/extern_fun_signature_with_pinned_is_error}.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_pin_unpin_basic.scoop`
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/{gc_pin_unpin_move_stress_matrix,gc_handle_roundtrip,gc_handle_token_roundtrip_callback_basic}.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_handle_stale_callback_token_is_error.scoop`
  - `cargo clippy --all-targets -- -D warnings`
- 下一步：回写 `TODO.md` 完成记录，检查工作区 diff，然后创建 `P6-T01` 提交并停止。
- 已完成 `TODO.md` 回写：`P6-T01` 标题已标记为 `[DONE]`，完成记录已写入改动范围、决策、验证与闭合说明。
- 当前只剩 git 提交流程：检查状态/差异/最近提交风格，创建 `P6-T01` 提交并停止。
