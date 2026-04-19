# 本轮执行计划

说明：按安全与协作要求，这里记录的是可审阅的执行依据、风险判断与步骤计划，不写不可审计的内部推理细节。

## 目标

完成 `TODO.md` 中第一个未完成任务，并在完成后立即停止。本轮根据现有交接状态，目标任务预计为 `T4009a`：拆掉 `Task<T> / Executor` 的 hard-coded handle ABI 绑定。

## 已知前情

1. 上一轮实现已将编译器 HIR/lowering、LLVM codegen、runtime C、sysroot/stdlib、runtime tests 与部分 fixtures 切换到泛型 `Task<T>` 对象 ABI。
2. 已验证：
   - `cargo build`
   - `cargo build -p scoopc`
   - `cargo run -q -p scoop -- test`
   - `cargo test --all`
   均已通过。
3. 最新提交 `6443fb5 [T4008R] Review effect lowering unification` 已被检查过，交接结论是没有必须先修复的 pre-existing issue。

## 本轮计划

1. 先检查当前工作树与 `TODO.md` / `PLAN.md` / 相关文件状态，确认首个未完成任务仍是 `T4009a`。
2. 跑 `cargo clippy --all-targets -- -D warnings`，如果出现 warning 或由此暴露出的真实问题，先修复到无 warning。
3. 视结果补充必要文档同步；重点确认是否需要更新 `README.md`、`SCOOP_RUNTIME.md`、`SCOOP_FULL_SPEC.md`，但不会为未实现的 `T4009b` 提前撰写超前文档。
4. 更新进度文件：
   - 在 `TODO.md` 中将 `T4009a` 标记为完成。
   - 在 `PLAN.md` 中记录本轮完成内容、验证命令、实现边界。
   - 在本文件中补记关键进展与最终结果。
5. 检查 git diff，确保只包含本轮需要提交的内容。
6. 提交 git commit，提交信息使用 `[T4009a] ...` 风格。
7. 停止，不继续处理后续任务。

## 风险与边界

1. 若 `clippy` 暴露的是独立缺陷且会影响 `T4009a` 的“完成”定义，本轮直接修复并纳入同一任务。
2. 若发现当前任务仍依赖缺失特性或与规范不一致，不能用 workaround 通过；必须把缺口改写进 `TODO.md` / `PLAN.md`，调整依赖顺序后提交并停止。
3. 交接中提到 `async { ... }` 某条路径仍保守写成 `Task<Int>`。当前初步判断这更接近 `T4009b` 的完整 async/poll/step 泛化边界；本轮会复核是否会影响 `T4009a` 的完成判定，并在 `PLAN.md` 中写清边界。

## 当前进展

1. 已复核：
   - `git log -1 --oneline --stat` 仍是 `6443fb5 [T4008R] Review effect lowering unification`
   - `TODO.md` 中首个未完成任务仍为 `T4009a`
2. `cargo clippy --all-targets -- -D warnings` 已通过，无新增 warning。
3. 文档快速扫描结果：
   - `SCOOP_FULL_SPEC.md` 当前对 `Task<T>` 的主叙事已是 general task object / manual polling 方向，没有明显残留旧 `*_int` task ABI。
   - `sysroot/task.scoop` / `sysroot/core.scoop` 已改到新的对象 ABI。
   - `stdlib/task.scoop` 仍保留 `Task<Int>` 级别组合子与 executor v0 适配，这与 `T4009b` 之前的阶段边界一致，暂不单独扩面。
4. 完整 fixture 回归 `cargo run -q -p scoop -- test` 正在执行中；待其结束后继续跑剩余收尾。

## 当前结果

1. 质量门禁已全部通过：
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo run -q -p scoop_tools -- spec-fixtures check`
   - `cargo run -q -p scoop -- test`（`fixtures: ok (1072)`）
   - `cargo test --all`
2. 本轮结论：
   - `T4009a` 可判定完成：`Task<T>` / `Executor` 已切到 GC-managed 对象 ABI，`spawn/join` / `taskCreate` / `onComplete` 不再依赖旧 handle ABI symbol。
   - `SCOOP_FULL_SPEC.md` / `SCOOP_RUNTIME.md` 当前主叙事已与 `T4009a` 对齐，无需额外文档改写。
   - `async { ... }` 最小 immediate-resume 路径里保守写死的 `Task<Int>` 仍存在，但它属于 `T4009b` 要继续定型的 poll/step/adapter 边界，不影响本轮“拆掉 handle ABI 绑定”的完成判定。
3. 待执行的最后步骤：
   - 已更新 `TODO.md` / `PLAN.md` 状态
   - 已检查 diff 与空白错误
   - 待提交 `[T4009a] ...`
   - 提交后停止
