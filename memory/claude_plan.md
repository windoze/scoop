# 当前执行计划

## 约束说明

- 本文件记录可共享的执行计划、关键决策和进度更新；不会记录不可公开的逐步内部推理。
- `TODO.md` 是任务顺序、完成状态和验收要求的权威来源。
- 本次只完成 `TODO.md` 中第一个标题未带 `[DONE]` 的任务，完成后提交并停止。

## 当前任务

- 已识别第一个未完成任务：`P2-T05R`。
- 任务范围：复审 `tools/README.md` 重写结果，确认覆盖所有 P1 Python 脚本，并确认旧 fixture runner / `scoop_tools` 入口无残留。
- 最近提交是 `[P2-T05] Record plan completion`，未明确留下与 `P2-T05R` 直接相关的未完成问题。
- 若复审发现具体文档缺口，则修复 `tools/README.md`；否则只更新任务 bookkeeping。

## 复审发现

- `tools/README.md` 已列出当前 `tools/*.py` 下全部 8 个 Python 脚本。
- 常用命令块包含每个 P1 Python 脚本入口。
- Shell helper 段落覆盖当前 3 个 `tools/*.sh` 文件。
- `tools/README.md` 的旧入口定向搜索未发现 `scoop_tools`、`scoop test`、`cargo run -p scoop -- test` 或 `test-fixtures` 残留。
- 无需修改 `tools/README.md`；本轮实际变更仅为 `TODO.md` 完成记录与本计划文件更新。

## 验证与记录

- `cargo fmt` 通过。
- `cargo clippy --all-targets -- -D warnings` 通过。
- `tools/README.md` 旧入口反向搜索无命中。
- `tools/README.md` 新脚本入口搜索显示 8 个 P1 Python 脚本均出现在脚本清单和命令块中。
- `TODO.md` 已将 `P2-T05R` 标记为 `[DONE]` 并追加完成记录。
- 完整 Rust 测试与完整 fixture 套件未重跑，因为本 review 仅修改 markdown/task bookkeeping，且前一任务已有完整绿色结果，本轮没有代码变更。
