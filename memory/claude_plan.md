# 本轮执行计划

## 目标
- 按照 `TODO.md` 的顺序，只完成第一个未完成任务，然后停止。

## 初始步骤
1. 检查最新一次 Git 提交，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如果该任务过大，拆分为可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前要执行的任务。
5. 运行相关测试与必要的 lint / 检查，修复发现的问题。
6. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况或依赖调整。
7. 提交本轮修改，随后停止。

## 说明
- 这里记录的是可审阅的执行计划与进度摘要，不包含内部草稿式推理。
- 在确认最新提交和任务内容后，我会把更具体的实施步骤补充到本文件。

## 当前进展
- 已检查最新提交：`1aa5e2bf8284b395e6ac1e02f470d9c51da1dc66`（`[T0147c-3c] 清零剩余结构性 clippy warning`）。提交信息中未显式记录需要先处理的遗留问题，因此无需在本轮进入额外预修复分支。
- 已读取 `TODO.md` 与 `PLAN.md`。
- 当前第一个未完成任务是 `T0147c`：`Float sysroot API 与 builtin 方法路由：resolver / typecheck / runtime / codegen`。
- 已完成主要实现：
  - `sysroot/core.scoop`：`Float64/Float32` 已声明为 `Hashable, ToString`，并补齐 `toInt()/toString()/hash()/abs()/isNaN()/isInfinite()`。
  - `resolve/scopes.rs` + `typecheck/expr/call.rs`：Float builtin member 调用已接入最小白名单与返回类型检查。
  - `runtime/c/scoop_runtime.c` + `runtime/c/scoop_runtime_api.h`：已新增 Float `toString()` / `toInt()` 的 runtime 符号，并预留 `scoop_string_to_float64`。
  - `llvm/codegen/*`：已新增 Float runtime symbol / ABI 声明，`toString()/toInt()/hash()/abs()/isNaN()/isInfinite()` 与 direct `print/println(Float)` 的 codegen 路径已补齐。
  - 测试：已新增 typecheck fixture 与 LLVM 单测；定向 LLVM 单测已收敛为 `float_builtin_methods_lower_to_runtime_calls_and_hash_bits` 并通过。
- 最终验证已完成：
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过。
  - `cargo test --all` 通过。
  - `cargo run -p scoop -- test` 通过（`fixtures: ok (853)`）。
- 收尾待办：
  - 更新 `TODO.md` / `PLAN.md` 为 DONE 状态。
  - 提交本轮修改并停止。

## 针对 T0147c 的细化执行步骤
1. 盘点现有 Float 相关实现：检查 sysroot、resolver、typecheck、runtime、LLVM codegen 中是否已有 `Float32/Float64` 基础类型与部分 builtin 支持。
2. 根据现状决定是否需要拆分任务；若任务可在本轮完整实现，则直接实现，不改写 `TODO.md` 结构。
3. 补齐 sysroot 声明与对应的 builtin 路由：
   - `toInt()` / `toString()` / `hash()` / `abs()` / `isNaN()` / `isInfinite()`
   - 覆盖 `Float64` 与 `Float32`
4. 补齐 runtime C API 与 LLVM 符号声明，确保 codegen 对上述方法调用不会 panic。
5. 增加或更新测试/fixtures，覆盖最小 Float builtin 方法调用路径。
6. 运行 `cargo fmt --all`、`cargo clippy --workspace --all-targets --message-format short -- -D warnings`、`cargo test --all`、`cargo run -p scoop -- test` 验证。
7. 更新 `TODO.md`、`PLAN.md`、本文件，提交本轮更改并停止。
