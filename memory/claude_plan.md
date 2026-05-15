# 执行计划

本文件记录本次执行的简明计划与进度，不包含内部推理细节。

## 初始计划

1. 读取 `TODO.md`，定位第一个未以 `[DONE]` 标记的任务。
2. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；若存在且构成当前任务前置条件，则在 `TODO.md` 中体现依赖关系。
3. 阅读当前任务涉及的代码、文档与测试，确认需求、约束、依赖与验证标准。
4. 以最小且正确的改动完成该任务；若遇到阻塞当前任务的真实缺口或缺陷，则先修复，或在 `TODO.md` 中加入最小前置任务并停止。
5. 运行与该任务直接相关的验证，再运行要求的全量质量检查（至少包括 `cargo clippy --all-targets -- -D warnings`，以及任务要求的测试）。
6. 更新进度文档：将任务在 `TODO.md` 中标记为 `[DONE]` 并补全完成记录；仅在阶段计划发生变化时更新 `PLAN.md`。
7. 检查工作区改动，按要求提交本次任务相关的全部未提交文件，并停止，不进入下一个任务。

## 进度记录

- 已创建本计划文件。
- 已读取 `TODO.md`，确认首个未完成任务为 `P4-T01d`：引入 method-level `@Intrinsic("name")` 与可枚举 intrinsic 表机制。
- 已检查最近一次提交 `bc019359 [P4-T01c] Allow bodied intrinsic nominal methods`；提交摘要未显示与 `P4-T01d` 直接相关且需先插入的新前置任务。
- 已检查当前工作树，确认存在一批未提交的 `P4-T01d` 相关改动（前端元数据、named intrinsic 表、codegen 通路、runtime dummy symbol 与 fixtures）。当前策略改为：先验证这批未提交实现是否完整，再补足缺口并按“恢复同一任务后统一提交”的要求一并提交。
- 已完成最小验证集：`named_intrinsic` 相关 LLVM/unit tests 与新增 run-pass/typecheck fixtures 均通过。
- 已跑 `tests/fixtures/run-pass` 全量；结果未出现新的 `P4-T01d` 回退，仍是仓库既有两个失败：`extern_native_aggregate_return_direct_indirect_parity.scoop`、`sync_gc_release_task_like_object_basic.scoop`。
- 已跑 `cargo test -p scoopc llvm_tests -- --nocapture`，当前 harness 仍返回 `0 passed; N filtered out`，因此实际 owner coverage 仍以命中的 `named_intrinsic` 测试为准。
- 已发现并修复一个本任务内质量门问题：`cargo clippy --all-targets -- -D warnings` 在 `crates/scoopc/src/llvm/codegen/intrinsics/named.rs` 报 `too_many_arguments`；已通过删除未使用的 `expected` 字段与 MIR lowering 冗余参数完成小重构，待重新验证。
- 已在修复后重新跑 owner 覆盖与 `clippy`，结果通过：
  - `cargo test -p scoopc named_intrinsic -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/intrinsic_named_method_dummy_ir_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/intrinsic_named_runtime_fun_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/intrinsic_named_fun_body_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/intrinsic_named_fun_unknown_table_entry_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/intrinsic_named_fun_without_allow_intrinsic_is_error.scoop`
  - `cargo clippy --all-targets -- -D warnings`
- 已将 `P4-T01d` 在 `TODO.md` 中标记为 `[DONE]`，并补写改动范围、核心决策、验证结果以及与 `PLAN.md` / `MANAGED_ABI.md` 的闭合说明。

## 当前执行细化

1. 阅读 `PLAN.md` 中 P4 前置 II 对 `P4-T01d` 的阶段约束，并检查现有 `@Intrinsic` 注解、sysroot gate、HIR lowering 与 LLVM intrinsic 代码结构。
2. 设计并实现 method-level intrinsic metadata 与编译器 intrinsic 表：
   - 前端只接受表内名字；
   - method-level intrinsic 与 type-level intrinsic 语义解耦；
   - 表项可审计，包含 lowering 模式与 RuntimeCall 理由。
3. 接通 codegen 调用路径，让标注了 `@Intrinsic("name")` 的方法调用按表执行 lowering。
4. 添加最小验证样例：dummy IR entry、dummy runtime entry、未知名字失败、有 body 失败、缺少 `@file:AllowIntrinsic` 失败。
5. 跑任务要求验证与 `cargo clippy --all-targets -- -D warnings`；若发现阻塞问题，先确认是否必须作为新前置任务写回 `TODO.md`。
6. 完成后更新 `TODO.md` 的 `[DONE]` 标记与完成记录；仅在阶段计划变更时更新 `PLAN.md`；最后提交 git commit 并停止。

## 当前判断

- 从代码与 fixture 状态看，`P4-T01d` 的主体可能已基本落地，但还未经过本轮完整验证，也尚未回写 `TODO.md`。
- 需要重点确认：
  - method-level 与 top-level `@Intrinsic("name")` 是否都能透传到 HIR/MIR/codegen；
  - dummy IR / RuntimeCall 两条通路是否都稳定通过；
  - 失败诊断是否与 fixture 预期一致；
  - 现有 run-pass / LLVM / clippy 是否无回退。

## 当前状态

- `P4-T01d` 已完成，剩余步骤只有：
  1. 检查最终工作树，确认本次任务涉及文件已纳入；
  2. 以 `P4-T01d` 风格写 commit message 提交；
  3. 停止，不进入 `P4-T01e`。
