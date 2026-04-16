# 当前执行计划

说明：以下内容记录的是可公开的执行依据、判断摘要和分步计划，用于追踪本次任务推进；不包含逐字内部思维。

## 目标

按照 `TODO.md` 的顺序执行首个未完成任务；若最近一次提交提到已有问题，则先修复这些问题；完成后更新 `TODO.md`、`PLAN.md`、本文件，并提交一次 Git commit，然后停止。

## 初始执行步骤

1. 检查最近一次提交的提交信息与变更摘要，确认是否提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位首个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖和既有拆解。
4. 如任务过大，先在 `PLAN.md` / `TODO.md` 中拆分为更小的可执行子任务，并以第一个子任务作为本次目标。
5. 实现本次目标，并同步检查是否暴露新的规范偏差或真实缺陷。
6. 为改动补充或更新测试，运行相关测试与必要的质量检查。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
8. 提交 Git commit，提交信息应清晰描述本次完成内容，然后停止，不进入下一个任务。

## 执行约束

- 不接受以规避方式通过测试；若发现规范缺口或真实实现边界，必须先把该问题前置到 `TODO.md`。
- 不回退用户已有改动；若工作区存在无关改动，仅在理解其影响后与之共存。
- 变更后需尽量确保 `cargo fmt`、相关测试及必要时的 `cargo clippy --all-targets -- -D warnings` 通过。

## 待确认信息

- 最近一次提交是否显式提到尚未修复的问题。
- `TODO.md` 中首个未完成任务的范围、依赖和可测试方式。

## 当前进展（2026-04-16）

- 最近一次提交为 `01dc83f5ddfa99868789072fd65f40b3291f49b0`，提交信息仅为 `Update plan`，未在提交信息中直接声明需要先修复的既有代码问题。
- `TODO.md` 中首个未完成任务是 `T3008`：将 full-state-machine frame 接入 GC typed alloc，并统一 `state` / continuation ABI 到 `addrspace(1)`。
- 当前工作区除本文件外还存在用户侧改动：`run_agent.sh`。本轮不触碰该文件。

## T3008 已确认问题

- 运行 `cargo run -p scoop -- run tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop` 当前失败，报错为 LLVM module verification：
  - `@scoop_continuation_alloc(ptr %0, ptr @scoop.effect.step...)`
  - 形参期望 `ptr addrspace(1)`，实参却是 native `ptr`
- 直接原因已确认：
  1. `codegen_handle_expr_via_state_machine` 仍使用 `malloc` 分配 raw frame。
  2. `emit_effect_step_function` 的 `state` 形参仍声明为 native `ptr`。
  3. `runtime_abi.rs` / continuation struct 已把 `state` 视为 GC-managed pointer，LLVM 侧 frame/continuation ABI 未完全对齐。

## T3008 实施方案

1. 将 effect frame LLVM 布局改为真正的 GC object：
   - 在 frame struct 前置 `ScoopGcObjectHeader`。
   - 为每个 handle frame 生成独立 type descriptor global。
   - trace 合同基于 frame struct 自动计算 bitmap，覆盖 `resume_gc_ref` 与所有 user slots 中的 GC ref。
2. 将 frame 分配路径从 `malloc` 改为 `scoop_alloc_typed(type_desc, size)`：
   - 只清零 header 之后的 payload 区域，避免覆盖 runtime 初始化的对象头。
3. 统一 ABI 到 `addrspace(1)`：
   - step function 的 `state` 参数改为 GC pointer。
   - continuation LLVM struct 中的 `state` / `resume_gc_ref` 槽位改为 GC pointer 表示。
   - 所有相关调用点同步对齐。
4. 如果 ABI 调整导致 runtime 测试中的旧 `ScoopContinuationStepFn` 两参签名不再合理，本轮一并改成三参签名。

## 本轮验证目标

- `cargo run -p scoop -- run tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop`
- `cargo run -p scoop -- run tests/fixtures/run-pass/try_catch_raise_runtime_error_basic.scoop`
- `cargo check -p scoopc`
- `cargo run -p scoop --features llvm -- test`

## 当前执行结果（2026-04-16，更新）

- `T3008` 发现实际需要拆分后，已按可执行子目标完成 `T3008a`：
  1. effect frame 改为 GC-managed typed object。
  2. step function / continuation `state` ABI 统一到 `addrspace(1)`。
  3. continuation LLVM struct 的 `state` / `resume_gc_ref` 槽位改为 GC pointer 表示。
  4. runtime 测试中的 continuation step 函数签名已统一到三参 ABI。
- 已通过验证：
  - `cargo check -p scoopc`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/try_catch_raise_runtime_error_basic.scoop`
- 已回收 24 个仅因 `ptr` / `ptr addrspace(1)` verifier 失败而临时 `EXPECT: fail` 的 run-pass fixtures。
- `cargo run -p scoop --features llvm -- test` 不再首先报 verifier error；继续运行后会进入 `effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`，表现为重复打印 `inner_catch` / `0`，对应的是已存在于 `TODO.md` 的 `T3014` handler-stack / arm-outside-scope 语义缺口，而不是本轮 ABI 修复残留。
- 已同步更新 `TODO.md` / `PLAN.md`：将当前完成内容落为 `T3008a [DONE]`，并把下次入口明确为 `T3008aR [TODO]`。

## 收尾动作

1. 复查工作区 diff，确认仅包含 `T3008a` 实现与文档同步。
2. 提交一次 Git commit，描述 `T3008a` 的 ABI / typed alloc 修复与 fixture expectation 收口。
3. 停止，不进入下一任务。
