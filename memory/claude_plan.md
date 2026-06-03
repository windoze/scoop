# 当前执行计划

## 范围与约束

- 目标：只完成 `TODO.md` 中第一个标题未带 `[DONE]` 的任务，然后停止。
- 任务来源：以 `TODO.md` 为唯一任务顺序和验收来源；`PLAN.md` 只在阶段级计划确实变化时更新。
- 不做开放式历史问题清扫；只处理会阻塞当前任务、使当前任务行为无效，或在验证中暴露且未被明确排期的失败。
- 如果当前任务被具体缺口阻塞，将在 `TODO.md` 插入最小必要前置任务，保持当前任务未完成，提交后停止。
- 验证顺序：先 `cargo fmt`，再 `cargo clippy --all-targets -- -D warnings`，再按任务需要运行 Rust 测试和 fixture 套件；完整套件使用不少于 30 分钟超时。

## 步骤

1. 读取 `TODO.md`，找出第一个标题未带 `[DONE]` 的任务，并读取其要求、依赖和验证项。
2. 检查最近提交信息是否明确提到与该任务直接相关的未完成问题；如有，将其纳入任务或作为前置项记录到 `TODO.md`。
3. 针对该任务阅读最小必要代码和测试上下文，确认应修改的位置。
4. 实现当前任务，优先采用最小正确改动，不引入规避方案或 fixture-only hack。
5. 添加或更新最小相关测试/fixture，覆盖任务要求和已修复的同类问题。
6. 按要求运行格式化、lint、相关测试；必要时运行完整测试和 fixture 套件。
7. 若验证发现未排期失败，修复它或在 `TODO.md` 中加入正确顺序的最小任务；不得把当前任务标为完成。
8. 完成后更新 `TODO.md`：给任务标题加 `[DONE]`，填写完成记录和验证结果；仅当阶段计划变化时更新 `PLAN.md`。
9. 更新本文件记录关键进度变化。
10. 检查 git 状态和 diff，提交本次任务相关全部变更，提交信息使用任务编号前缀。
11. 停止，不继续处理下一项任务。

## 当前状态

- 已读取 `TODO.md`，第一个未完成任务是 `T2-02-R：Review T2-02`。
- 最近提交为 `[T2-02] Address LIR callables by id`，直接对应当前 review，无额外未完成事项提示。
- 审查发现 `EntryRef` 仍保存 `StableLirCallableKey`，并在 LLVM emit/main wrapper 通过 `callable_by_lir_key` 查找 callable body；这是 T2-02 要求切换到 `LirCallableId` 的 live lookup 路径。
- 当前修正计划：将 `EntryRef` 的 callable 身份改为 `LirCallableId`，入口解析在 LIR 边界保留 id，LLVM emit/main wrapper 与相关测试改用 `callable_by_id`。
- 已实施入口路径修正；额外把 codegen 与 facts builder 中按 root FQN 扫描 `callables` / `callable_symbols` 的 T2-02 相关查找改为先解析 `LirCallableId` 再 `.get(&id)`。
- `cargo clippy --all-targets -- -D warnings` 首次运行发现 `callable_id` helper 可见性不足；已将其调整为 layout 子模块可见。
- 验证已通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、T2-02 targeted grep、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。
- 已更新 `TODO.md`，将 `T2-02-R` 标记为 `[DONE]` 并记录 review 修正与验证结果。
- 已提交代码、TODO 和计划记录，提交为 `e1195bdc [T2-02-R] Review LIR callable id lookups`。
- 当前任务已完成；停止前只需确认最终工作区状态。
