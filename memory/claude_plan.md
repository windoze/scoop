# 当前执行计划

说明：本文件记录可审计的执行计划、关键决策和进度更新，不包含隐藏推理过程。

## 本轮任务：T1-01-R

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 选择第一个未完成任务。
2. 仅检查与所选任务直接相关的最近提交信息，不做开放式历史问题排查。
3. 复核 `T1-01` 新增的 `LirArtifact` / `CodegenInput` 字段、导出、`llvm` feature 门控和零行为变化要求。
4. 如 review 发现与验收直接相关的问题，做最小正确修正。
5. 先运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`，再按任务要求和变更范围运行构建、测试与 fixture 验证。
6. 成功后更新 `TODO.md`，给 `T1-01-R` 标题加 `[DONE]` 并填写完成记录。
7. 检查 git 状态、diff 和近期提交，提交本任务相关变更，然后停止。

## 本轮进度记录

- 已确认第一个未完成任务为 `T1-01-R：Review T1-01`。
- 最近提交 `d268ad5b Add per-task review tasks to P1 TODO` 未提到与 `T1-01-R` 直接相关的未完成实现问题。
- 已复核 `LirArtifact` / `CodegenInput` 的精确匹配使用点，确认新类型仅在定义和 `pipeline` re-export 出现，尚未进入运行路径。
- 已补充 `facts`、`mir`、`entry` 过渡字段说明，明确 P2/T1-06 的移除或替换方向。
- 验证中发现 `cargo build -p scoopc --no-default-features` 失败，根因是现有 `single_cone`、`tool_commands` 和若干 tests/helpers 无条件引用 LLVM-only API；该问题直接影响本 review 的“非 llvm 构建不破”验收，已纳入本任务修复。
- 已将 `single_cone` 模块、LLVM artifact emission helper、LLVM-only frontend helpers/tests 正确挂到 `feature = "llvm"`，并修正 no-default 下 `TypeEnv` 绑定警告。
- 验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo clippy -p scoopc --all-targets --no-default-features -- -D warnings`；`cargo build -p scoop -p scoopc`；`cargo build -p scoopc --no-default-features`；`cargo test --all --all-targets`；`cargo test -p scoopc --all-targets --no-default-features`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。
- 已更新 `TODO.md`，将 `T1-01-R` 标记为 `[DONE]` 并填写完成记录。

## 上一轮记录：T1-01

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 判定第一个未完成任务。
2. 查看最近提交信息，只在其明确提到与当前任务直接相关的未完成事项时纳入当前任务或作为 `TODO.md` 前置项。
3. 阅读当前任务关联的规格、代码和测试，确认要求、依赖与验证命令。
4. 完整实现第一个未完成任务；如发现阻塞当前任务的缺失特性、规格不匹配或测试失败，优先修复，或在 `TODO.md` 插入最小前置任务后停止。
5. 运行格式化、lint 和相关测试；若代码发生变更，再按要求运行完整测试与 fixture 套件。
6. 更新 `TODO.md`：完成时给任务标题加 `[DONE]` 并填写 completion record；只有阶段级计划变化时才更新 `PLAN.md`。
7. 检查工作区差异，提交本次任务相关全部变更，然后停止，不继续下一个任务。

上一轮进度：

- 已创建初始计划文件，下一步读取任务列表并定位第一个未完成任务。
- 已确认第一个未完成任务为 `T1-01：新增 LirArtifact / CodegenInput 类型`。
- 最近提交 `d5e0b0ad Pivot to structural fact refactor; archive fact-unify plan` 未明确提到与 T1-01 直接相关的未完成修复。
- 当前任务执行策略：只新增过渡类型与模块导出，保持行为不变；随后按基线运行格式化、lint、测试与 fixture 验证。
- 已新增 `crates/scoopc/src/pipeline/lir_artifact.rs`，并在 `pipeline/mod.rs` 中按 `llvm` feature 导出 `CodegenInput` 与 `LirArtifact`。
- 验证已通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`（fixtures: ok 1664）。
- 已更新 `TODO.md`，将 T1-01 标记为 `[DONE]` 并填写完成记录；下一步检查 diff 并提交。
