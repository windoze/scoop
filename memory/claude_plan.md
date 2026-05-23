# 当前执行计划

## 目标

按 `TODO.md` 的权威顺序完成本轮第一个未标记 `[DONE]` 的任务：`P7-T05-b-0：发布 LIR-owned class ctor init body contract`。完成后更新 `TODO.md` 与 `TODO-6.md`，运行要求的验证，提交并停止。

## 步骤

1. 查看最新提交摘要，确认是否有与 `P7-T05-b-0` 直接相关的未完成事项需要并入本任务。
2. 定向阅读 class ctor、LIR facts、LIR facts builder、effect-lowered IR、mir_body args 与 dependency gate 相关代码，明确当前 ctor init body 由 HIR lowering 支撑的位置。
3. 在 LIR/LIR facts 或明确 base narrow contract 中发布 class ctor init body/init callable 的 authoritative 表达，覆盖 selected ctor、ordered params、delegation、property-param assignment、property initializer、init block 和 secondary body 的执行所有权。
4. 调整 LLVM class ctor production 路径，使其只负责对象分配、实参求值/rooting、调用 LIR-owned init contract 与效果传播检查，不再读取 `ctor.body.as_ref()` 或直接 `codegen_block_value(body)` lowering HIR ctor body。
5. 调整 `mir_body/args.rs` 等 helper，使 class ctor 参数与 source payload 消费来自已发布合同，不在 LLVM 中按 HIR side table 猜测或补齐。
6. 扩展 `dependency_gate`，阻止 production class ctor 路径重新出现 `ctor.body.as_ref` 和 `codegen_block_value(body)` 等 HIR body lowering residual。
7. 运行任务要求的验证：`cargo fmt`、`cargo run -p scoop_tools -- dependency-gate`、`cargo test -p scoopc_lir_facts`、`cargo test -p scoopc --no-default-features llvm_codegen_stage`、`cargo test -p scoopc --no-default-features llvm::codegen`、`cargo test -p scoopc llvm::codegen`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
8. 若验证发现未排期失败，修复或按规则更新 `TODO.md` 添加最小前置任务并停止；若通过，将 `P7-T05-b-0` 标记 `[DONE]` 并填写完成记录。
9. 检查工作区差异，提交本任务全部相关变更，然后停止。

## 进度

- 已读取 `TODO.md` 和 `TODO-6.md`；本轮第一个未完成任务是 `P7-T05-b-0`。
- 已写入本轮执行计划。
- 已查看最新提交：`33515264 [P7-T05-b] Schedule class ctor init contract prerequisite` 与当前任务直接相关，作为本任务背景处理。
- 当前工作区仅有本计划文件变更。
- 已完成定向代码调查：`llvm/codegen/class_ctor.rs` 当前仍从 `hir::MonoClassInit` / `hir::ClassCtor` 直接读取 ctor params、default args、delegation、init steps 和 secondary body；`scoopc_lir_facts` 尚无 class ctor init contract。
- 实施方向保持不变：新增 LIR-owned class ctor init body facts/source contract，并将 LLVM class ctor invoke 路径改为按该 contract 执行。
- 已完成首轮实现编辑：新增 `LirClassCtorInitFacts` / `LateLoweredClassCtorInitBody`，LIR builder 发布 ctor init bodies，LLVM class ctor invoke 改从 LIR contract 读取 params、super/delegation 与 init steps，并新增 dependency gate 规则。
- 已运行 `cargo fmt`；下一步执行针对性测试/编译并修复错误。
- 验证中发现完整 `run-pass` 只有 `sysroot_atomic_basic.scoop` 失败，原因是某些 codegen-only 泛型 class ctor init body 不在 LIR facts 中。
- 已补充 LLVM base narrow contract：`LlvmStageBaseContext` 从合并后的 class init index 发布 class ctor init bodies，LLVM class ctor 查询优先用 LIR facts/program，缺失时使用 base narrow contract；`sysroot_atomic_basic.scoop` 与完整 `run-pass` 已通过。
- 下一步重跑任务要求的剩余验证矩阵并修正任何 warning/error。
- 已完成全部任务验证：dependency gate、`scoopc_lir_facts`、no-default `llvm_codegen_stage`、no-default `llvm::codegen`、default `llvm::codegen`、完整 `run-pass`、clippy `-D warnings`、`git diff --check` 均通过。
- 已更新 `TODO.md` 与 `TODO-6.md`，将 `P7-T05-b-0` 标记为 `[DONE]` 并填写完成记录。
- 下一步检查最终 diff/status，提交本任务变更并停止。
