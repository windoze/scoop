# 执行计划与进度记录

说明：按要求先记录执行计划与进度。这里记录的是可审计的高层推理、步骤分解、发现与决策，不包含逐字内部思维。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 初始执行计划

1. 检查最新一次 Git 提交信息，确认是否提到任何现存问题。
2. 如果最新提交明确提到需要修复的现存问题，先定位并修复这些问题，再继续任务流。
3. 阅读 `TODO.md`，识别第一个未完成任务。
4. 阅读 `PLAN.md`，理解当前计划、依赖关系与上下文。
5. 判断该任务是否过大：
   - 如果可直接完成，则开始实现。
   - 如果过大，则把任务拆分为更小子任务，并更新 `PLAN.md` 与 `TODO.md`，随后执行拆分后的第一个子任务。
6. 实现任务，必要时补充或重构代码，以保证实现符合规范而不是依赖临时绕过方案。
7. 运行相关验证：
   - 至少运行与改动直接相关的测试；
   - 如有必要，运行更广泛的测试与质量检查，如 `cargo test`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。
8. 若过程中发现规范缺口、实现缺陷或依赖缺失：
   - 不绕过；
   - 先把缺陷转化为更前置的 `TODO.md` 任务并更新 `PLAN.md`；
   - 视情况提交变更后停止。
9. 任务完成后：
   - 更新 `TODO.md`，标记任务完成；
   - 更新 `PLAN.md`，反映当前状态；
   - 更新本文件记录结果与测试情况；
   - 提交 Git commit；
   - 停止，不继续下一个任务。

## 进度日志

- 已创建本文件并写入初始计划。
- 已检查最新提交：`[T3010b2b] Close post-suspend tail validation`。提交消息本身未额外标注需要先修复的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，确认当前首个未完成任务是 `T3010R`。
- 已开始执行 `T3010R` 的生产代码审查。
- 审查发现一个与 `T3010R` 目标直接冲突的真实残留：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中，`HandleStateOp::VarRef` 仍使用 `codegen_expr_in_expected_context(...).unwrap_or(CgValue::unit())`。
  - 这意味着 emitter 仍在依赖“VarRef 失败后吞掉并回退为 unit”的容错路径，与“生产 op 必须是独立可执行 op 或明确 no-op marker”的目标不符。
- 当前修复计划：
  1. 移除 `HandleStateOp::VarRef` 的吞错 fallback，让 unsupported standalone value ref 直接暴露为 codegen 错误，而不是伪执行成功。
  2. 修正对应的过时注释，明确 `VarRef` 不再承担 fragment-only 伪执行角色。
  3. 补一条定向测试，锁定纯 statement call 的 callee 不会再生成 standalone `VarRef` fragment op。
  4. 运行定向测试、全量 Rust 测试与 clippy；如有必要再跑 LLVM fixture 验证。
- 已完成代码修复：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
    - 删除 `HandleStateOp::VarRef` 的 `unwrap_or(CgValue::unit())` fallback。
    - 更新注释，要求 standalone `VarRef` 必须独立可执行。
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs`
    - 收紧 `source_plan_keeps_only_whole_call_for_pure_statement_args_and_pure_if_condition`，新增对 pure statement callee 不得生成 `VarRef` fragment 的断言。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc source_plan_ -- --nocapture`
  - `cargo test -p scoopc runtime_raise_boundary_ir_branches_between_inactive_continue_and_active_dispatch -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test`
- 验证结果摘要：
  - 定向测试、全量 Rust 测试与 clippy 全部通过。
  - LLVM fixture suite 仍停在既有 stale `EXPECT: fail`：`tests/fixtures/run-pass/continuation_resume_continuation.scoop`。该问题已由 `TODO.md` 中现有任务 `T3017` 跟踪，不是本轮新回归。
- 文档状态：
  - 已更新 `TODO.md`：`T3010R` 标记为完成并记录复审结论。
  - 已更新 `PLAN.md`：补入 `T3010R` 的修复/验证记录，并把下一项推进到 `T3011`。
