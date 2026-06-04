# Claude Execution Plan

当前任务：`TC-01-R：Review TC-01`。

范围约束：本次只完成 `TODO.md` 中第一个未完成任务。若发现阻塞当前 review 的缺陷或未调度失败，先修复或在 `TODO.md` 中插入最小必要前置任务，然后提交并停止；不推进到 `TC-02`。

执行计划：

1. 在确定当前任务后检查最新提交和工作区状态，确认是否存在与 `TC-01-R` 直接相关的未完成事项或未提交变更。
2. 按 `TC-01-R` 的关注点审查 `TC-01` 结果：确认 `lift.rs` lift 链为全函数、无 `Result`/`invalid_lift` 输入错误出口，占位失败已上移到 MIR→LIR guard。
3. 执行并核对任务要求的 grep 检查：`lift.rs` 中的 `Result<|invalid_lift`，`effect_lowered/{lift,instruction}.rs` 中的占位/escape 相关标记，以及 `scoopc_lir/src/effect_lowered` 中的 `lir_.*_to_mir|_fqn`。
4. 阅读关键实现与测试位置，确认 plain 与 effect-step callable body 都由完整 `LirExecutableBody` 承载，且没有 LIR→MIR 反向 shim、句柄→FQN 反转或 no-op 容忍。
5. 按 §9 基线顺序验证：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。完整测试和 fixture 使用至少 30 分钟超时。
6. 若所有审查与验证通过，更新 `TODO.md`：将 `TC-01-R` 标题标记为 `[DONE]`，补充完成记录；同步更新本计划文件的进度。
7. 提交本次 review 相关改动后停止。

当前进度：已完成 `TC-01-R` 审查与 §9 验证基线。`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py` 均通过。`TODO.md` 已将 `TC-01-R` 标记为 `[DONE]` 并写入完成记录。下一步检查 diff/status 后提交本次 review 改动并停止。
