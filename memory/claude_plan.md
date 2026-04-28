# 执行计划

## 约束说明

- 按用户要求，本文件在执行任何仓库探查命令前先创建。
- 由于尚未读取仓库当前状态，下面先记录预执行计划；读取 `TODO.md`、`PLAN.md`、最新提交说明后再补充精确任务与实现细节。
- 我不会写出逐字内部推理，但会持续记录可审计的计划、判断依据、关键发现、改动步骤与进度。

## 预执行步骤

1. 查看最新一次提交，确认是否提到需要优先修复的既有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，核对该任务的上下文、依赖与既有计划。
4. 如第一个未完成任务过大，则将其拆分为更小子任务，并同步更新 `TODO.md` 与 `PLAN.md`；本轮只执行拆分后的第一个子任务。
5. 实现当前目标任务，同时在过程中留意任何既有缺陷、规格不匹配、回避性实现或阻塞项；若发现，先修复，或按要求把前置修复任务插入 `TODO.md` 并停止。
6. 运行相关测试，并根据结果继续修复直至通过。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态、依赖变化、验证结果。
8. 提交本轮改动并停止，不继续处理下一个任务。

## 进度记录

- 已检查最新提交 `b08ca2c8f25ccc85152c800b73d3c828021c9232`，提交标题为 `[T5000j2R] Review when pattern MIR boundary`，提交正文未额外挂出需要优先修复的既有问题。
- 已读取 `TODO.md` / `PLAN.md`，当前第一个未完成任务是 `T5000j3 扩展更多 higher-order / closure / object-init / top-level-init 场景到 production MIR 主线`。
- 初步判断：`T5000j3` 任务面过大，应拆分后执行首个子任务。
- 已调查 `crates/scoopc/src/llvm/codegen/mir_body.rs`、`crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/llvm/reachability.rs`、`crates/scoopc/src/mir/{inline,closure_simplify,pass_view}.rs`，当前关键发现如下：
  - production MIR bridge 已支持一部分 raw materialized MIR：`Use` / `Unary` / `Binary` / `Direct Call` / `PatternMatch` / `PatternExtract` / `TopLevelRef` 等；
  - 但 canonical raw non-generic body 选择目前仍被 `raw_non_generic_pattern_candidate(...)` 限制为“含 pattern 的 body”；
  - 这意味着即便 raw non-generic body 已落在当前 MIR bridge 支持子集内，只要它不含 pattern，也不会被 production 主线选中；
  - `TopLevelRef` 已在 MIR bridge 与 reachability 扫描中显式支持 `object_inits`、`top_level_consts`、`top_level_immutable_values` 与 `top_level_vars`，因此 `top-level-init / object-init` 比 `closure/fun-value/MakeClosure` 更适合作为第一刀；
  - `CallKind::Closure` / `CallKind::FunValue`、`MakeClosure`、`CaptureBox*` 目前在 MIR bridge 中仍明确返回 unsupported，说明 higher-order / closure 需要后续独立子任务，不应和 init 场景混在本轮一起做。

## 当前拟定拆分

1. `T5000j3a`：扩展 `top-level-init / object-init` 相关的 raw non-generic callable body 到 production MIR 主线。
   - 重点检查 `emit.rs` / `reachability.rs` 的 canonical body 选择是否仍过窄；
   - 为 production codegen 增加 top-level immutable / object-init 场景回归；
   - 若证明确实是选择边界问题，则修复并验证不会把不支持形状错误地推进到 MIR bridge。
2. `T5000j3b`：扩展 higher-order / closure 简化后可发布形状到 production MIR 主线。
   - 只处理已经依赖 shared facts / pass artifacts 收缩到可发布形状的场景；
   - 不把 closure/fun-value 分析重新塞回 LLVM backend。
3. `T5000j3R`：汇总 review，确认新增覆盖继续依赖 materialized MIR / summary / escape facts。

## 当前执行计划

1. 先用测试确认 production path 在 `top-level-init / object-init` 场景下是否仍退回 HIR-compatible body。
2. 如果确认存在 gap，则先更新 `TODO.md` / `PLAN.md` 将 `T5000j3` 拆成 `j3a/j3b`，本轮执行 `j3a`。
3. 为 `j3a` 实现代码修改，并补 production regression tests。
4. 运行相关测试、`cargo clippy --all-targets -- -D warnings`，再更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 完成状态

- 已完成 `T5000j3a`：
  - canonical raw non-generic body 选择已从“仅 pattern body”扩到“pattern body + init 相关 `TopLevelRef` body”，实现位于 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/emit.rs` 与 `crates/scoopc/src/llvm/reachability.rs`；
  - `reachability` 已同步消费 `top_level_vars` / `object_inits`，确保 init 相关 raw body 的 canonical body 选择与 canonical reachability 扫描一致；
  - `raw_materialized_mir_body_requires_hir_compat_boundary` 现经 `raw_materialized_mir_terminator_is_supported(...)` 保守拒绝 `Return { value: None }`，避免 generic MIR 仍以隐式尾表达式约定表示返回值时，被 production raw MIR bridge 错降成默认值；
  - 已新增 `top-level immutable init`、object value init、closure fallback、implicit tail-return fallback、non-init/non-pattern helper fallback 与 ctor-call `Todo` reachability fallback 回归测试，覆盖本轮扩张与边界保护。
- 本轮排查中遇到的两个既有问题均已通过边界修正解决，而不是靠缩窄 fixture/workaround 绕过：
  - `effect_escape_continuation_indirect_perform_statement_container_matrix.scoop` 暴露了 implicit tail-return raw MIR body 误发射的问题；
  - `fun_call_add_basic.scoop` 暴露了“所有 raw non-generic body 一并纳入 canonical 选择”会把普通 helper 误推到 unsupported MIR bridge 的问题。
- 最终验证已顺序通过：
  - `cargo test -p scoopc production_codegen_ -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`（`fixtures: ok (1202)`）
- 下一步按 `TODO.md` 顺序应进入 `T5000j3aR Review：确认 init 场景扩张只是放宽 canonical MIR 覆盖，而非把分析责任倒灌回 backend`。
