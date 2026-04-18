# 执行计划

## 约束说明

- 先写入本计划文件，再执行任何命令或代码。
- 由于实现过程依赖仓库当前状态，下面计划分为“先检查再决策”的步骤；细化内容会在读取 `TODO.md`、`PLAN.md`、最近一次提交说明及相关代码后继续更新。

## 初始执行步骤

1. 检查最近一次 Git 提交，确认提交说明中是否提到已知遗留问题或需要顺手修复的问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，核对该任务的既有规划、依赖关系和优先级。
4. 检查当前工作树状态，识别是否存在用户未提交但与当前任务相关的改动，避免误覆盖。

## 决策规则

1. 如果最近一次提交暴露了前置缺陷，先修复这些缺陷，再进入 `TODO.md` 的任务。
2. 如果第一个未完成任务过大或依赖不清：
   - 拆分为更小的子任务；
   - 更新 `PLAN.md`；
   - 在 `TODO.md` 中替换或补充子任务，并确保顺序符合依赖；
   - 本轮只执行新的第一个子任务。
3. 如果执行过程中发现规范不匹配、语言特性缺失、实现边界不完整或任何“只能靠绕过”的情况：
   - 立即把该缺陷显式加入 `TODO.md`；
   - 将被阻塞任务移到依赖项之后；
   - 更新 `PLAN.md` 记录阻塞原因；
   - 提交这些计划性修改并停止本轮。

## 本轮目标

- 完成且仅完成当前最高优先级的一个待办任务（或其拆分后的第一个子任务）。
- 为该任务补齐必要实现、测试与文档更新。
- 更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md`。
- 使用清晰的 Git 提交信息提交本轮成果后停止。

## 完成标准

- 实现符合规范，无临时绕过或 fixture-only hack。
- 相关测试通过，且满足无警告要求；至少包含：
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - 与当前任务直接相关的测试命令
- 如果任务影响面较大，再补充必要的全量测试。

## 待检查项

- 最近一次提交的说明及可能遗留问题。
- `TODO.md` 中第一个未完成任务的具体定义。
- 该任务是否需要拆分。
- 是否存在阻塞该任务的规范差异或缺失特性。

## 进度记录

- 已完成：初始化计划文件。
- 已完成：检查最近一次提交 `754ae225679e352d2a17b891beedad10677f4e95`（`[T3016g] Fix resumed-body raise propagation through finally`）；提交说明本身未额外挂出新的“仍待修”遗留问题。
- 已完成：读取 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务为 `T3016gR`：Review resumed-body raise-after-resume 的 cleanup/propagation 合同。
- 已完成：初步核对本轮相关改动范围，最近一次实现仅涉及：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
  - `tests/fixtures/run-pass/effect_resume_finally_body_raise_after_resume.scoop`
- 已完成：复审生产代码，确认 `resume(...)` 之后再次 outward raise 走统一 cleanup/propagation 合同，而不是新增 immediate-resume / fixture 专用旁路。
- 已完成：验证 `cargo test -p scoopc cleanup_propagate_ir_restores_propagating_state_after_shared_finally_exit -- --nocapture`、3 条目标 fixture、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 与 `cargo test --all`，全部通过。
- 已完成：更新 `TODO.md` / `PLAN.md` 状态，本轮 `T3016gR` 完成记录已整理完毕并准备提交。

## T3016gR 细化执行计划

1. 审查 `state_machine_emitter.rs` 中 dispatch loop、cleanup propagate、cleanup done 与 terminator 相关路径：
   - 确认新的修复只是在 shared cleanup path 上保留并恢复 propagating `state_tag`；
   - 确认 terminal completion 仍由统一 `STATE_TAG_HANDLE_RETURNED` / `STATE_TAG_FUNCTION_RETURNED` 合同驱动；
   - 确认 outward propagation 继续通过统一 dispatch/cleanup 出口返回，而不是 immediate-resume 专用 shortcut。
2. 代码检索：
   - 检查是否出现 fixture 名称、helper 名称或 runtime 特判；
   - 检查是否新增按 immediate-resume / `resume(...)` 形状分流到独立 cleanup/completion 逻辑的入口。
3. 验证：
   - 定向运行与本修复直接相关的测试；
   - 运行 `cargo test --all`；
   - 运行 `cargo clippy --all-targets -- -D warnings`；
   - 运行 `cargo fmt --check`。
4. 若复审未发现问题：
   - 将 `T3016gR` 标记完成；
   - 更新 `PLAN.md` 记录审查结论和下一项任务；
   - 更新本文件的进度；
   - 提交本轮更改后停止。

## 本轮验证结论

- `T3016g` 的修复没有引入 immediate-resume 专用 cleanup/completion 旁路。
- shared `finally` cleanup 的 outward propagation 仍经统一 dispatch loop 协调，只是在进入 cleanup 前保留 propagating `state_tag`，以防 shared cleanup 普通出口把传播路径误写成 terminal sentinel。
- 普通完成路径与 outward propagation 路径分别继续通过既有 completion-tag 合同与非终止 `state_tag` 合同收口，没有 fixture-only workaround。
- 下一项待办应推进到 `T3016h`。
