# 本轮执行计划

## 已知上下文

- 最新提交 `86c33ac` 已检查，提交信息未指出需要先处理的遗留问题。
- 根据现有进展，本轮目标任务是 `TODO.md` 中首个未完成项 `T2003u5d2`。
- 该任务的核心代码与回归样例已基本实现，当前剩余工作是全量验证、任务文档更新、提交并停止。
- 本轮必须只完成一个任务；若验证暴露新的真实缺陷，需要先修复该缺陷并继续围绕 `T2003u5d2` 收尾，不能越过到下一个任务。

## 执行步骤

1. 读取 `TODO.md`、`PLAN.md`、`memory/claude_plan.md` 当前状态，并核对工作区改动，确认本轮目标仍是 `T2003u5d2`。
2. 读取最新提交信息，确认不存在额外要求先修的遗留问题。
3. 运行剩余验证命令：
   - `cargo run -p scoop -- test`
   - `cargo run -p scoop --features llvm -- test`
   - `cargo clippy --workspace --all-targets -- -D warnings`
4. 若验证失败：
   - 判定是否为 `T2003u5d2` 未完成导致的问题。
   - 若是，修复实现与测试，并重新执行相关验证直到通过。
   - 若发现规范不匹配或缺失前置能力，按照要求调整 `TODO.md` / `PLAN.md` 记录依赖与阻塞关系，但当前摘要判断大概率不需要。
5. 若验证通过：
   - 更新 `TODO.md`，将 `T2003u5d2` 标记为完成。
   - 更新 `PLAN.md`，记录本任务完成内容、测试结果与新增回归覆盖。
   - 更新本文件，记录关键步骤已完成和待提交事项。
6. 检查工作区差异，确认无临时调试残留。
7. 使用清晰提交信息提交本轮改动，建议为：`[T2003u5d2] Support immediate+escape while richer nested block-if replay`。
8. 提交后停止，不继续处理下一项任务。

## 当前判断

- 当前实现已通过完整验收，接下来只剩提交并停止。
- 下一轮首个未完成任务将变为 `T2003u5d3`，不在本轮处理范围内。

## 本轮已完成

- 已核对 `TODO.md` / `PLAN.md` / 最新提交与工作区状态，确认本轮目标是 `T2003u5d2`。
- 已完成剩余验证：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 已确认无遗留调试字符串残留。
- 已更新任务文档：
  - `TODO.md` 已将 `T2003u5d2` 标记为完成，并记录 richer nested `block/if` direct / indirect 回归。
  - `PLAN.md` 已补充本轮完成说明，并把“当前下一步”调整为 `T2003u5d3`。

## 待提交摘要

- 代码：放宽 mixed-arm immediate+escape while nested-path 到 `block/if` richer nested 子集；扩展 while mixed prefix / scan / tail helper 沿 source-path 穿过 block 进入 if；修复 indirect skip 分支误重放 then-tail。
- 回归：删除 build-fail `effect_resume_mixed_escape_while_is_error`；新增 run-pass `effect_resume_mixed_escape_pre_immediate_while_nested_block_if` 与 `effect_resume_mixed_escape_post_immediate_while_nested_block_if_indirect`。
- 提交信息建议：`[T2003u5d2] Support immediate+escape while richer nested block-if replay`

## 进度记录

- [x] 在执行命令前写入本轮计划。
- [x] 核对任务与工作区状态。
- [x] 完成剩余全量验证。
- [x] 更新 `TODO.md` / `PLAN.md` / `memory/claude_plan.md`。
- [ ] 提交并停止。
