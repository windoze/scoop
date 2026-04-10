## 当前执行计划（续）

无法提供逐字内部思考过程；这里记录可审计的执行计划、关键判断和进度，便于随时检查。

### 目标

本轮只完成 `TODO.md` 中首个未完成任务：`T0147c-2d Clippy 基线清理：typecheck expr 主干签名收口`，然后停止。

### 已知状态

1. 最新提交 `ad94bf4 [T0147c-2c]` 未在提交信息中暴露需要先处理的遗留问题。
2. `ExprInferInputs` 已在 `mod.rs`、`infer.rs`、`call.rs`、`stmt.rs`、`entry.rs` 中接入一轮，`infer.rs` 的 `clippy::too_many_arguments` 已清零。
3. 当前剩余工作集中在：
   - `crates/scoopc/src/typecheck/expr/call.rs` 的最后一个 `too_many_arguments`
   - `crates/scoopc/src/typecheck/expr/entry.rs` 的多处 `too_many_arguments`
   - 这轮重构引入的少量 `unused variable`

### 执行步骤

1. 先收掉 `call.rs` 剩余的单个超参函数，优先复用 `ExprInferInputs` 或新增轻量请求对象。
2. 再处理 `entry.rs` 的超参函数，尽量把重复传递的共享类型检查上下文收口到 1 到 2 个轻量结构里，避免扩大改动面。
3. 跑定向命令确认 `clippy::too_many_arguments` 全部清零，并顺手修掉本轮引入的未使用变量。
4. 任务完成后执行格式化与验收：
   - `cargo fmt --all`
   - `cargo clippy -p scoopc --all-targets --message-format short -- -W clippy::too_many_arguments`
   - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`
   - `cargo test --all`
   - `cargo run -p scoop -- test`
5. 若 `-D warnings` 被后续任务范围内的既有基线问题阻塞，明确记录阻塞事实；但本任务必须完整消除 `too_many_arguments`。
6. 任务完成后更新 `TODO.md`、`PLAN.md`、本文件，并提交一次 git commit，然后停止。

### 当前判断

- 不在本轮扩展处理 `result_large_err`、`private_interfaces`、`dead_code` 等非 `T0147c-2d` 范围告警。
- 不回退任何既有用户改动，只在当前任务相关文件内做最小必要重构。

### 最新进度

- 已把 `call.rs` / `infer.rs` / `entry.rs` 的剩余 `too_many_arguments` 收口到上下文对象：
  - `call.rs` 新增 `EnumTypeSubstContext`
  - `entry.rs` 新增文件级 / class 级共享上下文以及 ctor 调用请求对象
  - `stmt.rs`、`val_pat.rs`、`when_exhaustiveness.rs`、`when_pat.rs` 的复用调用点已同步切换
- `cargo check -p scoopc --message-format short` 已恢复通过。
- `cargo clippy -p scoopc --all-targets --message-format short -- -W clippy::too_many_arguments` 已复核通过，当前输出中不再出现 `too_many_arguments`。
- 验收已完成：
  - `cargo fmt --all` 通过
  - `cargo clippy -p scoopc --all-targets --message-format short -- -W clippy::too_many_arguments` 通过，未再出现 `too_many_arguments`
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings` 已执行；失败点仅剩 `T0147c-3` 范围内的既有 `result_large_err` / `private_interfaces` / `dead_code` / `large_enum_variant` 等 warning
  - `cargo test --all` 通过
  - `cargo run -p scoop -- test` 通过（`fixtures: ok (852)`）
- 下一步只剩更新 `TODO.md` / `PLAN.md`、检查变更、提交 git commit，然后停止。
