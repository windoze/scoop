# 执行计划

## 约束与目标

- 本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在开始实现任务前，必须先检查最新提交是否提到任何既有问题；若有，先修复这些问题。
- 若当前首个未完成任务过大，需要先将其拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
- 实现后必须运行相关测试，并尽量满足无告警要求，包括 `cargo clippy --all-targets -- -D warnings`。
- 过程中若发现规范不匹配、缺失功能或阻塞项，不能绕过；必须先把问题写入 `TODO.md` 和 `PLAN.md`，提交后停止。
- 完成后需要更新 `TODO.md`、`PLAN.md`，并创建一次 git 提交。

## 当前已知执行步骤

1. 查看最新一次 git 提交的信息，确认是否提到待修复的既有问题。
2. 查看 `TODO.md`，定位第一个未完成任务。
3. 查看 `PLAN.md`、必要时查看相关说明文档，确认该任务上下文、依赖和预期行为。
4. 判断任务是否可以在本轮完整交付。
5. 如果任务过大：
   - 设计可执行的子任务拆分。
   - 更新 `PLAN.md` 说明拆分后的执行顺序与原因。
   - 更新 `TODO.md`，让首个子任务成为新的第一未完成项。
   - 仅执行该首个子任务。
6. 如果任务可直接执行：
   - 阅读相关代码、测试、规范与现有实现。
   - 实现缺失功能或修复问题。
   - 补充或调整测试。
7. 运行相关验证：
   - 至少运行与变更直接相关的测试。
   - 若变更影响范围较大，运行更全面的测试与 `cargo clippy --all-targets -- -D warnings`。
8. 更新文档状态：
   - 在 `TODO.md` 中将已完成任务标记为完成。
   - 在 `PLAN.md` 中记录已完成内容、剩余依赖和必要调整。
   - 按需更新本文件，记录关键进展与计划变更。
9. 检查工作区改动，确认没有误改或遗漏。
10. 使用清晰提交信息进行 git 提交，然后停止。

## 决策原则

- 若最新提交中明确提到遗留缺陷，则该缺陷优先级高于 `TODO.md` 当前任务。
- 若某项实现依赖尚未支持的语言特性、标准库能力或运行时行为，则先把缺口转化为新的前置任务，而不是做权宜处理。
- 若测试暴露现有实现与规范不一致，也视为必须先处理的项目问题。

## 进展记录

- 已创建本计划文件，准备开始检查最新提交与任务列表。
- 已检查最新提交 `52257b559af103ca10f8ffeea799068dc93d93b0`：
  - 提交标题为 `[T2003u4c2] Route multiple escape/nonresuming through unified plan`。
  - commit body 为空，未直接声明需要本轮先修复的遗留问题。
- 已定位 `TODO.md` 中第一个未完成任务：`T2003u4c3 [TODO] Effect：immediate+escape / site-matrix 路由切到统一状态机输入`。
- 下一步：读取 `T2003u4c3` 在 `TODO.md` / `PLAN.md` 中的完整定义，审查相关 LLVM effect lowering 代码与现有 unified-plan 实现，判断是否需要继续拆分。
- 已完成代码审查结论：
  - 本任务当前仍可在一轮内交付，不需要继续拆分 `TODO.md` / `PLAN.md`。
  - 现状是：
    - `ImmediateResumeWithEscapeSibling` / `ImmediateResumeWithEscapeAndNonResumingSiblings` 的主分发仍从 `scan_immediate_resume_site`、`scan_mixed_escape_direct_sites`、`scan_mixed_escape_indirect_sites` 做入口判定。
    - `matrix.rs` 的 immediate+escape site-matrix 主线也仍直接依赖上述扫描器。
    - 但 pure escape / multiple escape 路由已经具备 plan-driven helper，可作为本轮实现模板。

## 细化执行计划

1. 在 immediate+escape 路径上补 plan-driven 解析 helper：
   - 从 `HandleStateMachinePlan` 恢复 immediate arm 的唯一 direct perform site。
   - 从 plan 恢复 escape sibling 的 direct / indirect sites，并保留 source order 与 nested source-path。
2. 改造 immediate+escape 的分发入口：
   - 让 `nonresuming.rs` 调用改为传入 `state_machine_plan`。
   - 让 `mixed.rs` 中 `codegen_handle_expr_immediate_resume_with_escape_*` 的 direct / indirect / site-matrix 路由优先使用 plan-driven 结果，而不是旧扫描器。
3. 改造实际 emitter：
   - `mixed.rs` 的 direct / indirect emitter 改为从 unified plan 恢复 immediate site 与 escape sites。
   - `matrix.rs` 的 site-matrix emitter 改为从 unified plan 恢复 immediate site 与 direct/indirect matrix sites。
   - 旧 replay / cleanup helper 可以暂时保留，只替换它们的 source-of-truth。
4. 补单测：
   - 为 immediate+escape 的 plan-driven site 恢复增加至少一组 unit test，覆盖 nested control-flow / source order。
5. 运行验证：
   - `cargo test --all`
   - `cargo run -p scoop --features llvm -- test`
   - `cargo clippy --workspace --all-targets -- -D warnings`
6. 验证通过后：
   - 更新 `TODO.md` 将 `T2003u4c3` 标记完成并补完成说明。
   - 更新 `PLAN.md` 记录本轮进展，并把下一步推进到 `T2003u5`。
   - git 提交并停止。

## 执行结果

- 已完成代码修改：
  - `nonresuming.rs` 现已把 immediate+escape 路由显式传入 `state_machine_plan`。
  - `mixed.rs` 已新增 `resolve_immediate_resume_with_escape_sites_from_plan`，并让 immediate+escape 的 direct / indirect / site-matrix 路由优先消费 unified plan。
  - `matrix.rs` 的 immediate+escape site-matrix 主线已改为从 unified plan 恢复 immediate site 与 escape direct/indirect sites。
  - `state_machine_plan_tests.rs` 已新增解析层单测，覆盖 nested `if` 中的 direct / indirect mixed-site 恢复。
- 已处理 lint 收口：
  - legacy `scan.rs` 仍保留为辅助/参考实现，但已做最小范围 `dead_code` 收口，避免影响 `clippy -D warnings`。
  - immediate+escape 内部 emitter 因新增 `state_machine_plan` 参数触发的 `too_many_arguments` 已做窄范围例外标注。
- 已完成验证：
  - `cargo test --all`：通过
  - `cargo clippy --workspace --all-targets -- -D warnings`：通过
  - `cargo run -p scoop --features llvm -- test`：通过（`fixtures: ok (993)`）
- 正在收尾：
  - 待检查最终 diff，并更新 git 状态后提交。
