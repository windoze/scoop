# 执行计划

## 约束摘要
- 以 `TODO.md` 为唯一任务顺序与完成状态来源。
- 只完成第一个标题未带 `[DONE]` 的任务，完成后提交并停止。
- 若发现当前任务被具体前置问题阻塞，先修复该问题；若无法在本次完成，则把最小前置任务插入 `TODO.md`，提交后停止。
- 不用规避、弱化测试或改变规格来绕过实现缺口。
- 编辑前先建立上下文，变更后按要求运行格式化、lint、测试与夹具验证。

## 初始执行步骤
1. 读取 `TODO.md`，定位第一个未完成任务及其验证要求。
2. 检查最近提交是否明确提到与该任务直接相关的未完成事项。
3. 阅读当前任务涉及的代码、测试、规格或夹具，确认实现范围。
4. 如任务可直接完成，则进行最小正确实现并补充或更新测试。
5. 运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`。
6. 运行当前任务要求的相关测试；若需要，运行完整 `cargo test --all --all-targets` 与 `cargo run -p scoop -- test`。
7. 若所有相关验证通过，更新 `TODO.md`：给任务标题加 `[DONE]` 并填写完成记录。
8. 检查变更范围与 git 状态，提交本次任务所有相关变更。

## 当前状态
- 已读取 `TODO.md` / `TODO-7.md` 并确认第一个可执行未完成任务为 `P10-T04-c-1`。
- `P10-T04-c` 是已拆分的父任务占位，正文明确要求按 `P10-T04-c-1..4` 收口，且根索引当前下一项也指向 `P10-T04-c-1`。
- 最近提交为 `[P10-T06R] Review per-cone subprocess concurrent compilation driver`，与 `P10-T04-c-1` 的前置依赖一致，无需新增前置任务。
- 已把 LIR callable canonical 改为基于 body version 内容的 `body#h<hash>`，不再使用 program-local callable 下标。
- 已新增 `stable_lir_callable_key_ignores_program_local_callable_order` 单元测试，并通过 `cargo check -p scoopc --lib` 与该单测。
- 首轮完整 Rust 测试发现 `scoopc --lib` 中 3 个 effect-lowered dump 测试因 TypeId 越界失败；已定位为 canonical 编码错误使用 MIR `TypeStore`，并改为使用 effect-owned `MaterializedEffectFacts::types()`。
- 手工 `nm` 首次仍不一致，定位为 `LirCallableSymbolFacts.exported_symbol` 仍保存 MIR materialization 的旧 exported symbol，且 MIR non-generic stable template fallback 未使用 source-cone owner。已改为 LIR callable key 生成导出 ABI symbol，并让 MIR non-generic stable template key携带 source-cone identity。
- 重新 cold build `source_path_dependency_public_call` 后，consumer `main.o` 与 dep `scoop.o` 的 `dependencyValue` symbol 后缀均为 `h16644f5508fdcad1a359b25c324f6fae`，手工 `nm` 比对通过。
- 后续完整 Rust 测试只因 panic sentinel 行号基线随 MIR stable-template 修复移动而失败；已更新 `pipeline_user_visible_failure_policy` 的对应行号记录。
- 已更新 `TODO.md` / `TODO-7.md`，将 `P10-T04-c-1` 标记为 `[DONE]` 并记录实现与验证结果；下一任务为 `P10-T04-c-2`，本次不会继续执行。
- 最终验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`；手工 `nm` 比对。

## P10-T04-c-1 执行计划
1. 检查工作树状态，避免覆盖用户或并行代理的未提交改动。
2. 阅读 `stable_lir_callable_key`、`LateLoweredBodyVersionKey`、`StableLirCallableKey`、LLVM reachability 与 LIR facts 相关调用点。
3. 将 LIR callable canonical 从 `body#<program-local-index>` 改为基于 `LateLoweredBodyVersionKey` 的内容稳定标识，同时保留 readable path 的可读性。
4. 更新所有直接构造 `StableLirCallableKey` 的现场，消除任何依赖 program-local callable 顺序的 ABI symbol 生成路径。
5. 添加或调整测试，覆盖同一 callable 在不同 `LateLoweredProgram` 组合/顺序下生成相同 mangled symbol。
6. 运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo run -p scoop -- test`、`git diff --check`，并执行 `source_path_dependency_public_call` 的 `nm` 手工比对。
7. 验证通过后，同步更新 `TODO.md` 与 `TODO-7.md`：将 `P10-T04-c-1` 标记为 `[DONE]` 并写完成记录。
8. 检查 diff / status / recent log 后提交本任务变更并停止。
