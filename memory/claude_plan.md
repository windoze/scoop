# 当前执行计划

更新日期：2026-06-02

## 目标

完成 `TODO.md` 中按顺序出现的第一个未完成任务，完成后停止，不继续处理后续任务。

## 步骤

1. 读取 `TODO.md`，按规则识别第一个标题未带 `[DONE]` 的任务。
2. 检查该任务的依赖、完成要求和验证要求；必要时查看 `PLAN.md` 和最近提交，只限于判断当前任务相关上下文。
3. 根据当前任务定位相关代码、测试或文档，确认应修改的最小范围。
4. 实现当前任务；如果发现阻塞当前任务的缺失语言功能、规格不匹配或未安排失败测试，则按要求更新 `TODO.md` 插入最小前置任务并停止。
5. 运行格式化、lint、相关测试，并在需要时运行完整测试和 fixture 套件。
6. 将当前任务标题更新为 `[DONE]`，补全完成记录；仅当阶段级计划变化时才更新 `PLAN.md`。
7. 检查工作区差异，提交本次任务相关改动，然后停止。

## 当前状态

已读取 `TODO.md` 与 `TODO-3.md`。首个未完成任务是 `T3-04R：Review T3-04`，依赖 `T3-04A` 已标记完成；本次只执行该 review 任务。

## T3-04R 执行计划

1. 查看最近提交，确认是否有与 `T3-04R` 直接相关的未完成问题。
2. 审查 `T3-04`、`T3-04A0`、`T3-04A` 涉及的 fail-fast、fact-only、verifier、dependency gate 与 fixture 验证范围。
3. 如发现影响 `T3-04` 完成条件的缺口，直接修复并补充测试；如发现必须前置处理的新阻塞，则更新 `TODO.md` 后停止。
4. 运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`，再运行 `python3 tools/run_fixtures.py`；若代码发生变化，再按需要运行完整 Rust 测试。
5. 审查通过后，将 `T3-04R` 标题和任务索引标为 `[DONE]`，填写完成记录，并同步 `TODO.md` 主索引状态。
6. 检查差异并提交本次任务全部相关改动，然后停止。

## 审查阻塞记录

`T3-04R` 二次审查发现 `T3-04A` 后仍存在阻塞 `T3-04` 完成条件的残余缺口：P6 source-span intrinsic/direct-call side table、intrinsic FQN/root fallback、dispatch FQN/side-table 恢复、`readable_path()` root fallback、P4/P5 verifier 发布目标校验缺口，以及 dependency gate 覆盖缺口。

已将最小前置修复任务 `T3-04B` 插入 `TODO-3.md` 中 `T3-04R` 之前，并将 `T3-04R` 依赖更新为 `T3-04B`。本次不标记 `T3-04R` 为完成；接下来只验证文档/任务单更新并提交后停止。

## 验证记录

本次只修改 `TODO.md`、`TODO-3.md` 与 `memory/claude_plan.md`，未修改代码、测试或 fixture；因此跳过 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets` 与 `python3 tools/run_fixtures.py`。
