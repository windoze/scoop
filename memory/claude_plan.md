# 当前执行计划

1. 读取 `TODO.md`，按文件顺序定位第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交信息，确认是否存在与该任务直接相关的未完成事项；仅在其阻塞当前任务时纳入本轮处理或写入 `TODO.md` 作为前置任务。
3. 阅读当前任务涉及的代码、测试、规范或夹具，确定实现边界与验证要求。
4. 完整实现第一个未完成任务；如遇到无法规避的规范缺口或阻塞问题，按要求将最小前置任务写入 `TODO.md` 并停止。
5. 按顺序运行格式化、lint、相关测试，以及任务要求的完整验证；发现未安排的失败时修复或将其作为前置/跟进任务写入 `TODO.md`。
6. 更新 `TODO.md`：在完成任务标题前加 `[DONE]`，补充完成记录和实际验证结果；仅当阶段级计划变化时更新 `PLAN.md`。
7. 提交本轮所有相关更改，提交信息包含任务编号和简明说明。
8. 完成一个任务后停止，不继续处理后续任务。

## 进度记录

- 已创建初始执行计划，下一步读取 `TODO.md` 定位第一个未完成任务。
- 已读取 `TODO.md`，第一个未完成任务为 `TC-02-R：Review TC-02`。本轮只执行该 review 任务：检查最近提交相关性，核对 plain LIR 发射是否符合 TC-02 要求，运行指定 grep/验证命令，更新 `TODO.md` 完成记录并提交。
- 已检查最近提交，HEAD 为 `[TC-02] Record completion plan`，提交信息未声明与 `TC-02-R` 直接相关的未完成阻塞项。
- 已完成 TC-02 代码路径初审：plain 普通分支与 source closure 分支从 `LirExecutableBody` 的 state/statement/terminator 发射；旧 `mir::...` body match、raw MIR route gate、`lir_*_to_mir` 均未检出。`_fqn` 仍有命中，归属已排在后续的 `TC-04` FQN→句柄迁移，不作为本 review 的新增阻塞。
- 下一步按基线顺序运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、完整测试和 fixture 验证。
- 验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。
- 已将 `TODO.md` 中 `TC-02-R` 标记为 `[DONE]` 并写入完成记录；阶段级计划未变化，因此不更新 `PLAN.md`。下一步检查 diff/status 并提交本轮 review 变更。
