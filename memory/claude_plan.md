# 执行计划

## 当前状态

- 本次调用目标：根据 `TODO.md` 的顺序完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 约束：先识别任务，不做开放式历史问题扫查；如遇到阻塞当前任务的未排期缺陷，先修复或在 `TODO.md` 中插入最小必要前置任务并提交后停止。
- 说明：本文件记录可审阅的执行计划、进度和关键决策，不记录私有推理过程。

## 初始步骤

1. 阅读 `TODO.md`，按标题 `[DONE]` 前缀判断第一个未完成任务。
2. 查看最近提交，确认是否明确提到与该任务直接相关的未完成问题。
3. 阅读当前任务涉及的计划、规格、源码和测试上下文，仅限于完成当前任务所需范围。
4. 如任务可直接实施，进行最小正确代码/测试/文档修改。
5. 按要求先运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`，最后运行相关测试、完整 Rust 测试和完整 fixture suite；如仅文档变化且已有可复用绿色结果，则记录跳过原因。
6. 若所有验证通过，更新 `TODO.md`：在任务标题前加 `[DONE]` 并补充完成记录。
7. 检查 `git status`、`git diff`、最近提交，确认提交范围包含本任务相关改动且不回退他人改动。
8. 使用任务编号编写清晰提交信息并提交。
9. 停止，不推进下一个任务。

## 后续更新

- 已识别当前任务：`TC-03-R：Review TC-03`。
- 最近提交：`1cfb3dc0 [TC-03] Emit effect body statements from LIR`，直接对应本 review 范围，未在标题中提示额外未完事项。

## TC-03-R 执行计划

1. 复核 `TC-03` 相关 codegen 路径，确认 effect-step state 内语句来自 LIR `LirExecutableBody` / state-owned statements，不再经 MIR source slice 消费语句。
2. 按任务验收运行 grep：确认 `effect_lowered/body` 中 `.source_body()`、`.source_slices()`、`LateLoweredSourceBody`、`mir::Rvalue`、`mir::Statement` 语句消费残留清零。
3. 检查是否存在新增 `lir_*_to_mir`、占位、输入 `Result`/panic/expect 等与 TC-03 review 直接相关的反模式。
4. 如发现 TC-03 回归或阻塞问题，先修复；如发现无法在本任务内正确修复的前置缺口，则更新 `TODO.md` 插入前置任务并停止。
5. 验证通过后按基线顺序运行：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。
6. 更新 `TODO.md`：将 `TC-03-R` 标题标记为 `[DONE]`，补充 completion record。
7. 检查 diff 和最近提交，提交本 review 记录及必要改动，然后停止。

## TC-03-R 进度

- 已确认 `effect_lowered/body` 验收 grep 对 `.source_body()`、`.source_slices()`、`LateLoweredSourceBody`、`mir::Rvalue`、`mir::Statement` 无命中。
- Review 发现 TC-03 迁移后的 `used_locals` 只收集 LIR statements/Return/Branch，漏掉 published boundary operand contracts 中被 Call/Perform/Resume 终止器消费的 locals；这可能让 `codegen_lir_statement` 误跳过仅由 boundary payload/args/continuation 使用的 top-level refs。
- 已修复：在 effect body emitter 初始化 `used_locals` 时并入 callable published contracts 的 local consumers，包括 boundary operand sources、completion payload source、handle completion payload sources 和 frame-slot source locals；resume payload consumer locals 是写入目标，未计作旧值 use。修复保持语句发射 LIR-native，不回读 MIR terminator args。
- 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。
- 已更新 `TODO.md`：`TC-03-R` 标题标记为 `[DONE]` 并补充完成记录。
