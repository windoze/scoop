# 执行计划

说明：此文件记录可审计的执行计划、关键决策和进度；不记录不可公开的逐字内部推理。

## 当前目标

- 当前第一个未完成任务：`TC-04：FQN 引用改句柄`。
- 完整实现 codegen 中 callee / 符号 / 布局 live FQN 字符串查找到 `LirCallableId` / `NominalId` 句柄 deref 的迁移，完成验证，更新 `TODO.md`，提交一次 Git commit，然后停止。

## 步骤

1. 读取 `TODO.md`，确认第一个未完成任务及其依赖、验收条件和验证要求。
2. 查看最近提交信息，判断是否有与该任务直接相关的未完成事项需要纳入当前范围。
3. 读取任务相关代码、测试、规格文档，确定最小正确修改范围。
4. 若任务存在必须先处理的具体阻塞项，在 `TODO.md` 中加入最小前置任务并提交后停止；否则继续实现当前任务。
5. 按仓库风格实现代码和测试，避免规避规格或 fixture-only hack。
6. 依次运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、相关测试；必要时运行完整测试和 fixture 套件。
7. 更新 `TODO.md`：给已完成任务标题加 `[DONE]`，填写完成记录和验证记录。
8. 检查工作区 diff，提交本次任务相关改动，提交信息包含任务编号。
9. 停止，不继续处理下一个任务。

## 进度

- 已创建初始执行计划。
- 已读取 `TODO.md` 并确认首个未完成任务为 `TC-04`。
- 已检查最近提交：最新提交为 `TC-03-R` 审查记录，无直接要求并入 `TC-04` 的未完成事项。
- 已按验收 grep 定位需迁移项：`current_callable_fqn`、`lir_callable_id_for_root`、`program.callable(...)`、`abi_symbol_for_root(...)`、`published_signature_matches_hir_call`。
- 当前实现策略：保留 FQN 仅作为 LLVM 符号名、稳定命名和诊断文本；live callable 解析改用 `LirCallableId` / `LirCallableRef`，`program.callable(...)` 调用点改为 ID deref 或测试专用 root-to-id 辅助。
- 已完成第一轮代码迁移，并确认以下 grep 暂无命中：`program.callable(`、`lir_callable_id_for_root|abi_symbol_for_root|current_callable_fqn`、`published_signature_matches_hir_call`。
- `cargo fmt` 已通过。
- `cargo clippy --all-targets -- -D warnings` 已通过。
- 首次完整 fixture 发现 4 个失败：`deprecated_fun_call_warning_basic.scoop`、`member_call_struct_body_method_basic.scoop`、`overload_concrete_bug.scoop`、`entry_package_selects_correct_main`。
- 失败根因：当前 callable id 取自 body program，但 codegen 的 active LIR program 是 ABI program；跨 program 索引不一致导致读取错误 owner 的 call-site contract。
- 已修复：进入 plain/effect/closure callable 时优先按 active LIR program 的 root 解析 `current_lir_callable_id`，body program id 仅兜底；4 个失败 fixture 单独复测均通过。
- 修复后 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo build -p scoop -p scoopc` 已重新通过。
- 修复后完整验证通过：`cargo test --all --all-targets`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。
