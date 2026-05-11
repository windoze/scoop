## 执行计划摘要

说明：此文件记录可执行计划与进度更新，不包含内部推理细节。

### 初始计划

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交信息是否直接指出与该任务相关的未完成问题；若是，则将其视为当前任务一部分，或在 `TODO.md` 中补充为前置任务。
3. 阅读当前任务在 `TODO.md` 中的要求、依赖、验证标准与完成记录。
4. 如有必要，阅读 `PLAN.md` 以确认阶段级依赖，但不将其用作日常任务记录。
5. 检查工作区状态，识别与当前任务相关的未提交变更，避免覆盖他人工作。
6. 调研并实现当前任务所需的最小正确改动；如遇阻塞当前任务的真实缺陷或缺失能力，先修复，或将其以最小前置任务形式写入 `TODO.md`。
7. 运行当前任务要求的验证，包括相关测试、`cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`，以及任务明确要求的其它检查。
8. 更新 `TODO.md`：仅在任务真正完成时为任务标题添加 `[DONE]`，并补全完成记录；若出现阻塞，则写入新的前置任务并保持当前任务未完成。
9. 仅当阶段计划或依赖结构变化时更新 `PLAN.md`。
10. 提交所有本次任务相关改动，并停止，不继续下一个任务。

### 进度

- 已创建初始计划文件。
- 已读取 `TODO.md`，当前首个未完成任务为 `G8-T09`：runtime generic substrate 收尾、验证面迁移与 full regression 恢复。
- 已检查最近提交：`[G7-T08] Rebuild perform/handle/resume/Step_F lowering`；提交信息未额外声明一个必须先插入 `TODO.md` 的新前置问题。
- 已检查工作区：存在一批未提交改动，集中在 `G7-T08R` 收尾、runtime generic substrate 清理、验证面迁移、文档/缺口表更新与测试替换。

### 当前执行步骤

1. 复核当前未提交改动是否都属于 `G8-T09` 的延续现场，并补齐缺失的 runtime/test/doc 收尾。
2. 检查 `runtime/c`、活跃测试与文档中是否仍残留旧 continuation/effect policy 名字或过时叙述。
3. 运行 `G8-T09` 要求的验证矩阵：
   - `cargo check -p scoop_runtime`
   - `cargo check -p scoopc`
   - `cargo test -p scoop_runtime`
   - `cargo test -p scoopc`
   - `cargo test -p scoop`
   - `cargo test --all`
   - 以及必要的 `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings`
4. 若验证暴露当前任务范围内的问题，立即修复并重跑相关验证。
5. 完成后更新 `TODO.md` 的 `G8-T09` 完成记录，并在 `EFFECT_REFACTOR_GAPS.md` 中给出最终闭合状态。
6. 提交当前所有未提交文件，停止执行。

### 关键进展

- 运行 `cargo test -p scoopc` 时发现一个阻塞：`llvm::tests::effectful_funptr_call_uses_explicit_outcome_boundary` 失败，错误为缺少 continuation schema `k4` 的 `surface-resume owner dispatch contract`。
- 已通过检查 `dump-effect-lowered` 定位到根因：effectful funptr 动态调用发布了 callee continuation schema `k4`，但 `register_call_boundary_callee_wrapper_projection(...)` 在 `carrier_target_step_schemas` 为空时错误排除了 owner step 与 caller 同 schema 的候选，导致 `k4` 未进入 authoritative dispatch inventory。
- 已修复 `crates/scoopc/src/effect_lowered/ir.rs` 的候选筛选：当 wrapper continuation schema 与 caller continuation schema 不同但 owner step 相同，允许回收到 caller 的 authoritative continuation object。
- 已复跑定向回归 `cargo test -p scoopc llvm::tests::effectful_funptr_call_uses_explicit_outcome_boundary -- --exact --nocapture`：通过。
- 完整验证矩阵已复跑通过：`cargo fmt --check`、`cargo check -p scoop_runtime`、`cargo check -p scoopc`、`cargo test -p scoop_runtime`、`cargo test -p scoopc`、`cargo test -p scoop`、`cargo test --all`、`cargo clippy --workspace --all-targets -- -D warnings`。
- full regression 期间还发现并修复了第二个 blocker：plain `Resume` boundary complete path 误把 `frame_root` 提前清空，导致 payloaded `Continuation.resume(...)` 在 `p7_default_pipeline`/run-pass CLI 下提前终止；现已在 `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs` 中对 `Resume` complete tail 收紧这条 root-release 优化。
- 已将 `G8-T09` 在 `TODO.md` 中标记为 `[DONE]` 并补全完成记录。
- 下一步：检查最终工作树、创建任务提交并停止。
