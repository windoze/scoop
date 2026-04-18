# 本轮执行计划

## 目标

完成 `TODO.md` 中第一个未完成任务，然后停止；如果在执行前或执行中发现前置缺陷、规范不匹配或任务过大，则先调整 `TODO.md` / `PLAN.md`，记录依赖关系，并在本轮只处理新的最前置事项。

## 约束

- 先检查最新提交说明，若其中提到已有问题，需要先修复。
- 只执行一个任务或一个新拆分出来的首个子任务。
- 不以规避方案、fixture hack 或偏离规范的方式交付。
- 变更后必须更新 `TODO.md`、`PLAN.md` 和本文件，并提交 Git。
- 需要进行充分验证，至少覆盖相关测试、格式化和 `clippy` 无警告检查。

## 执行步骤

1. 查看最新提交信息，确认是否提到待修复的遗留问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，确认当前计划与任务依赖。
4. 判断该任务是否过大或是否被前置缺陷阻塞。
5. 若需要拆分或重排任务：
   - 更新 `TODO.md` 的任务顺序与依赖。
   - 更新 `PLAN.md` 记录原因与新的执行顺序。
   - 更新本文件记录本轮调整。
   - 完成一次提交后停止。
6. 若可以直接执行：
   - 实现任务。
   - 增补或调整测试。
   - 运行必要验证命令。
7. 完成后：
   - 在 `TODO.md` 标记任务完成。
   - 更新 `PLAN.md` 和本文件中的进度记录。
   - 提交本轮变更并停止。

## 进度记录

- 已写入初始计划，并检查最新提交：最新提交仅为 `[T3016k] Restore unified effect trace hook activation`，未额外声明新的待修遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，确认本轮首个未完成任务为 `T3016kR`，且无需继续拆分。
- 已完成 `T3016kR` 复审：
  - 审查了 `runtime_symbols.rs`、`runtime_abi.rs`、`effect/mod.rs`、`effect/state_machine_emitter.rs`。
  - 结论是 ordinary `perform`、runtime-error `Raise.raise` 与 unified suspend 共享 traceful activation 合同；剩余 plain `set_active` 仅用于 sysroot intrinsic / runtime ABI 测试，不属于 non-resuming effect 生产路径。
- 已完成验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_raise_trace_hook_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已完成任务状态更新；本轮将在提交后停止，下一项应推进到 `T3017`。
