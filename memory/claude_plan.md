# 执行计划

## 当前状态

- 当前任务：`C6-T01：全量回归与最终审计`。
- 最新提交：`87dac258 [C5-T01] Update spec and migration notes`，与当前 final review 直接相关但未显示新的未完成阻塞事项。
- 约束：一次只完成 `TODO.md` 中第一个未完成任务；完成后更新任务记录、验证并提交，然后停止。
- 说明：本文件记录可审查的计划、关键决策和进度；不记录不可公开的内部推理细节。

## 步骤计划

1. 运行 `cargo fmt`，确保格式化无差异或自动格式化完成。
2. 运行 `cargo build`。
3. 运行 `cargo test --all --all-targets`。
4. 运行完整 fixture suite：`cargo run -p scoop -- test`，使用足够长的超时。
5. 运行 `cargo run -p scoop_tools -- spec-fixtures check`。
6. 执行 CaptureBox final grep，确认 source/active fixture 无残留。
7. 执行 sealed-interface final grep，确认实现与 fixture 标记仍存在并可审计。
8. 执行 atomic-ref final grep，确认 direct HIR LLVM、effect-lowered LLVM 与测试覆盖仍存在。
9. 若全部通过，更新 `TODO.md` 将 `C6-T01` 标记为 `[DONE]` 并填写完成记录；阶段计划未变则不更新 `PLAN.md`。
10. 提交本任务相关改动；如果所有任务完成且仓库没有既有 `v0.1.0` tag，则创建 `v0.1.0`。

## 进度

- `cargo fmt`：已完成，无命令输出。
- `cargo build`：已通过。
- `cargo test --all --all-targets`：已通过，`scoopc` 测试段显示 `871 passed`。
- `cargo run -p scoop -- test`：已通过，`fixtures: ok (1405)`。
- `cargo run -p scoop_tools -- spec-fixtures check`：已通过，`spec fixtures: ok (1)`。
- CaptureBox final grep：`crates/scoopc/src sysroot tests/fixtures` 无输出，active source/fixture 无残留。
- sealed-interface final grep：实现诊断、audit 登记与 typecheck fixtures 均有命中。
- atomic-ref final grep：`sysroot` API/unsafe intrinsic、direct HIR LLVM lowering、effect-lowered LLVM lowering、Rust LLVM test 与 build/run/typecheck fixtures 均有命中。
- `TODO.md`：已将 `C6-T01` 标记为 `[DONE]` 并填写完成记录；`PLAN.md` 未更新。
