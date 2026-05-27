# 当前执行计划

## 范围

- 本次只处理 `TODO.md` 中第一个未完成任务。
- 已识别首个未完成任务：`P2-T07R`，复审 `SCOOP_FULL_SPEC.md` 更新，确认旧 fixture runner 入口无残留。
- 以 `TODO.md` 作为任务顺序、依赖、验证要求和完成记录的权威来源。
- 完成或阻塞当前任务后停止，不继续处理 `P2-T08`。

## 执行计划

1. 检查最近提交，确认是否存在与 `P2-T07R` 直接相关的未完成事项。
2. 查看 `SCOOP_FULL_SPEC.md` 中 P2-T07 修改区域，确认 spec doctest / fixture-suite 命令已切换到 Python 脚本。
3. 搜索 `SCOOP_FULL_SPEC.md` 中旧入口模式：`scoop_tools`、`cargo run -p scoop -- test`、`scoop test`、`test-fixtures`、`target/debug|release scoop test`、`cargo run -p scoopc -- test-fixtures`。
4. 搜索 `SCOOP_FULL_SPEC.md` 中新入口模式：`python3 tools/spec_fixtures.py` 与 `python3 tools/run_fixtures.py`。
5. 如复审发现遗漏，直接修正 `SCOOP_FULL_SPEC.md`，并按需要运行 `python3 tools/spec_fixtures.py sync/check`。
6. 按项目要求先运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`。
7. 运行任务相关验证：`python3 tools/spec_fixtures.py check` 与 `python3 tools/run_fixtures.py tests/fixtures/spec_doctest`。
8. 若本轮没有代码语义变更，完整 `cargo test --all --all-targets` 与完整 `python3 tools/run_fixtures.py` 可复用上一条完成记录的绿色结果，并在完成记录中说明跳过原因。
9. 将 `TODO.md` 中 `P2-T07R` 标题和索引状态标记为 `[DONE]`，并追加完成记录。
10. 更新本文件记录关键进展。
11. 检查 `git status`、`git diff`、最近提交，确认只提交本任务相关文件；如工作区已有无关变更，不回退、不纳入提交。
12. 以 `[P2-T07R] Review spec fixture command cleanup` 提交本任务变更，然后停止。

## 进度记录

- 已读取 `TODO.md`，确认 `P2-T07R` 是当前第一个未完成任务。
- 本计划已在执行 git/验证命令前写入。
- 最近提交为 `300992ac [P2-T07] Update spec fixture commands`，直接对应当前 review 任务，未显示额外未完成事项。
- 当前工作区存在无关变更/未跟踪文件：`run_agent.sh`、`CALLER_LOCATION.md`、`RTTI_REFINE.md`、`tools/__pycache__/`；本任务不会回退或提交这些文件。
- `SCOOP_FULL_SPEC.md` 旧入口模式搜索无命中；新入口 `python3 tools/spec_fixtures.py` / `python3 tools/run_fixtures.py` 有 3 处命中。
- 已复审最新提交对 `SCOOP_FULL_SPEC.md` 的 diff：三处 doctest fixture 命令已切换为 `python3 tools/spec_fixtures.py sync/check` 与 `python3 tools/run_fixtures.py tests/fixtures/spec_doctest`，无需修改 spec 正文。
- 验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py tests/fixtures/spec_doctest`。
- 已更新 `TODO.md`：将 `P2-T07R` 标记为 `[DONE]` 并追加完成记录；完整 `cargo test --all --all-targets` 与完整 fixture suite 因本轮仅 markdown/task bookkeeping 且无代码变更而复用最近绿色结果。
