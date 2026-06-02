# 当前执行计划

## 原则

- 以 `TODO.md` 为唯一任务排序和完成状态来源。
- 只处理第一个标题未带 `[DONE]` 的任务，完成后立即停止。
- 若遇到阻塞当前任务的实现缺口、规格不匹配或测试失败，优先修复；若无法在当前任务中合理修复，则把最小必要前置任务加入 `TODO.md` 并提交后停止。
- 不把例行进度写入 `PLAN.md`；只有阶段级计划、依赖或完成标准变化时才更新。
- 不回滚或覆盖非本次产生的用户改动。

## 步骤

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交信息，确认是否有与该任务直接相关的未完成事项。
3. 读取该任务涉及的代码、测试、文档和相关规范，明确验收要求。
4. 如任务可直接完成，进行最小且完整的实现；如存在必须先解决的具体前置问题，更新 `TODO.md` 记录该前置任务并停止。
5. 按要求先运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`，然后运行相关测试；需要全量验证时使用至少 30 分钟超时。
6. 若发现未安排的测试或 fixture 失败，修复它，或在 `TODO.md` 中加入合适的前置/后续任务且不把当前任务标记完成。
7. 完成实现和验证后，在 `TODO.md` 中给当前任务标题添加 `[DONE]` 并更新完成记录。
8. 检查 `git status`、`git diff` 和最近提交，确认只提交应包含的文件。
9. 使用符合项目风格的提交信息提交本次任务所有相关改动。
10. 停止，不继续处理下一个任务。

## 当前状态

- 已读取 `TODO.md` / `TODO-3.md`，首个未完成任务为 `T3-04E`。
- 最近提交 `cee978d2 [T3-04R] Schedule ctor intrinsic follow-up` 正是为该任务登记的直接相关前置缺口，应作为当前任务执行内容。
- 已完成第一轮结构编辑：P6 handoff/codegen 不再携带 `ctor_call_sites`；LIR facts 新增 class ctor call-site contract；旧 `reflection_type_args` source-path/span map 已删除；LLVM scalar bodyless intrinsic FQN fallback helper 已删除；dependency gate 已补对应守卫。
- `cargo fmt` 与 `cargo clippy --all-targets -- -D warnings` 已通过；期间补齐了 LIR fact dump/verifier字段更新与 class ctor call helper的 clippy 标注。
- 完整 Rust 测试首次通过后，fixture suite 暴露 scalar intrinsic metadata 上移、reflection SiteId fact、generic ctor fact 与 effect-lowered golden 更新需求；已完成对应修复，关键单项 fixtures 已恢复通过。
- 最后一轮单项失败已修复：value-overload reflection 不再发布空 type-arg fact；pure source payload scalar intrinsics 现在可从已发布 `intrinsic_callables` 精确读取 metadata。
- 最终验证已通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`（1664 checks）。
- `TODO-3.md` 已将 `T3-04E` 标记为 `[DONE]` 并写入完成记录；`TODO.md` 当前活跃任务已切到后续 `T3-04R`。
- 下一步：检查 git diff/status，提交本任务改动后停止。
