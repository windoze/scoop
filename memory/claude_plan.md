# Claude Plan

## 约束说明
- 不写入内部完整推理过程；这里记录可执行计划、关键决策与进度状态。

## 初始执行计划
1. 读取 `TODO.md`，把它当作索引，而不是任务细节来源。
2. 按索引顺序读取对应的 `TODO-Px.md`，定位首个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；若是，则将其作为当前任务的一部分或前置依赖处理。
4. 阅读与当前任务直接相关的代码、测试、规范与任务约束，确认最小正确改动范围。
5. 实现当前任务；如果遇到阻塞当前任务的真实缺口或规范不匹配，不做绕过，而是在对应 `TODO-Px.md` 中插入最小前置任务，并同步 `TODO.md` 后停止。
6. 运行与改动直接相关的验证；必要时补充更广泛验证，目标包括无警告通过 `cargo clippy --all-targets -- -D warnings`。
7. 更新 `memory/claude_plan.md` 记录关键进展；完成后在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]`，补全完成记录，并在需要时同步 `TODO.md` 与 `PLAN.md`。
8. 按任务号创建一次 git 提交，然后停止，不继续下一个任务。

## 进度
- 已写入初始计划，下一步开始定位首个未完成详细任务。
- 已确认首个未完成任务是 `P6-T02h`；`TODO-P6.md` 中已有未提交的 blocker 记录与 `TODO.md` 索引同步改动，需要保留并纳入本次工作。
- 已检查最近一次提交：`[P6-T02g] Publish carrier dynamic entry targets`，未显式留下必须先处理的相关未完成 issue。
- 已读取当前实现：`P5` 只为 pure caller call boundary 发布了 `consumed_runtime_error_case(target_state)`，state graph 中新增的 `LocalRuntimeError` synthetic state 仍只携带 `payload_tuple_ty`；LLVM `RefactorAbiQuery` 也只公开 `input_case_tag / payload_abi / target_state`，没有 terminal action。
- 已确认一个关键边界：带本地 `try/catch RuntimeError` 的 pure caller（如 `continuation_resume_answer_expression_basic.scoop`）当前不会生成 `LocalRuntimeError` synthetic state；因此 `P6-T02h` 处理的 `LocalRuntimeError` 路径可聚焦为“pure caller 无 outward/runtime-error catch 时的已发布终止语义”，预计应发布为显式 runtime-fatal contract，而不是让 backend 现场猜测。
- 当前执行方案：
  1. 在 `effect_lowered::ir` 中为 `LocalRuntimeError` 与 `consumed_runtime_error_case` 增补结构化 terminal action。
  2. 在 `materialize.rs` 中 authoritative 地生成该 terminal action，并在 `dump.rs` 中公开它。
  3. 在 LLVM refactor ABI query/type/layout 层发布对应 runtime terminal contract 与 fail-fast 校验。
  4. 视需要补充 runtime entry 声明/实现，使 terminal action 对 backend 是可直接 lower 的已发布 contract。
  5. 运行定向测试、fixture dump 与 `clippy`，随后更新 `TODO-P6.md` / `TODO.md` / `memory/claude_plan.md` 并提交。
- 已完成实现：
  - `effect_lowered` 已把 `LocalRuntimeError` / `consumed_runtime_error_case` 升级为显式 terminal contract，并在 dump 中公开 `RuntimeFatal(runtime_entry=scoop_runtime_error_fatal)`。
  - LLVM refactor ABI query 已发布对应 runtime entry symbol/type，并对 target state、payload、terminal action 做 fail-fast 校验。
  - runtime 已新增 `scoop_runtime_error_fatal(void* runtime_error)` 入口，当前实现为立即终止。
- 已完成验证：
  - `cargo test -p scoopc refactor_boundary_lowering`
  - `cargo test -p scoopc refactor_effect_lowered_stage`
  - `cargo test -p scoopc refactor_llvm_local_runtime_error_contract`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 收尾中：已将 `P6-T02h` 标记为 `[DONE]` 并同步 `TODO.md`，下一步检查工作树后创建本次提交并停止。
