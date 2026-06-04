# 当前执行计划

## 推理摘要

- `TODO.md` 是唯一的任务排序和完成状态来源；本次只处理第一个标题未带 `[DONE]` 的任务。
- 在读取任务前不做开放式历史问题扫描，避免偏离当前任务顺序。
- 如当前任务被具体缺陷、缺失语言特性或测试失败阻塞，将先修复该阻塞；若无法在本次完成，则把最小必要前置任务插入 `TODO.md` 并停止。
- 完成任务后需要更新 `TODO.md` 的任务标题为 `[DONE]`，填写完成记录，运行要求的格式化、lint、测试/fixture 验证，并提交 Git。

## 步骤计划

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并确认任务要求、依赖和验证要求。
2. 查看最新提交信息；仅当最新提交明确提到与当前任务直接相关的未完成问题时，将其纳入当前任务或作为前置任务记录到 `TODO.md`。
3. 针对当前任务检查相关代码和测试，确认实现边界，不做无关历史缺陷扫描。
4. 实施当前任务要求的最小正确代码或文档变更；如果发现必须先修复的具体阻塞，则按规则更新 `TODO.md` 并停止。
5. 按要求先运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`，随后运行相关测试；如代码有变更且需要完整验证，再运行完整 Rust 测试和 fixture 套件。
6. 若发现未被计划覆盖的测试或 fixture 失败，立即修复；无法修复时在 `TODO.md` 中加入最小必要前置/后续任务，且不把当前任务标为完成。
7. 更新 `TODO.md`：把完成的当前任务标题加上 `[DONE]`，并填写完成记录和验证结果；仅当阶段级计划发生变化时才更新 `PLAN.md`。
8. 检查 Git 状态、diff 和近期日志，确认只提交预期文件；按仓库风格创建清晰提交。
9. 本次只完成一个任务，提交后停止。

## 当前状态

- 已读取 `TODO.md`，第一个标题未带 `[DONE]` 的任务是 `TC-02：plain 路径（mir_body/）改 walk LIR 指令`。
- 已确认 §9 验证基线；最新提交 `1dd84e8c [TC-02-PRE2] Converge plain LIR fixture baseline` 是当前任务直接前置，不需要新增前置任务。
- 初查发现 `codegen_plain_callable_entry` 普通 plain 分支已经遍历 `LirExecutableBody`，但 `mir_body/` 仍有大量 MIR statement/rvalue/terminator helpers、route-safe gate 和 MIR 专用测试残留，当前任务的主要实现是清除这些残留并保证 LIR helpers 覆盖实际调用路径。
- 已删除已无生产调用的旧 plain MIR `codegen_mir_statement` / `codegen_mir_terminator` / `codegen_mir_rvalue` / `codegen_mir_call` 和 raw MIR route gate。
- 已把普通 plain 入口的返回类型、source span、materialized closure 判定和 composite transport 校验切到 `LirExecutableBody` / LIR header；仅 `plain.local_effect_control()` 分支仍按现有 effect/source-slice 路径取 MIR source body，该残留与后续 `TC-03` 的 effect 语句迁移直接相关。
- 已把 `mir_body/` 内剩余 source-slice 兼容层改走 LIR 发布的 `mir_source` 边界，直接 raw `crate::mir::*` body 类型 grep 清零。
- 已完成 §9 验证：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py` 均通过。
- 已在 `TODO.md` 将 `TC-02` 标为 `[DONE]` 并填写完成记录；下一步检查 diff/status 后提交。
