# 执行计划

## 说明

按安全与协作要求，这里记录可对外共享的任务分析、执行步骤、关键决策与进度，不记录原始内部推理细节。

## 当前目标

完成 `TODO.md` 中第一个未完成任务；若在检查最近提交、测试、实现或评审过程中发现既有问题，则先修复该问题或将其整理为当前任务的前置任务并更新计划与任务顺序。

## 执行步骤

1. 检查最近一次 Git 提交，确认是否明确提到待修复问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对现有计划与任务依赖。
4. 评估该任务规模：
   - 若可在当前轮完整完成，则直接实现。
   - 若过大或被既有缺陷阻塞，则将其拆分为更小前置任务，更新 `PLAN.md` 与 `TODO.md`，并只执行新的第一个子任务。
5. 在实现前阅读相关代码、测试、规范与最近变更，确认正确修改点。
6. 实现任务，必要时补充或整理注释、模块边界、README 等关联内容。
7. 运行相关验证：
   - 最小相关测试；
   - 必要时运行更广泛测试；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`（若本任务影响范围要求如此）。
8. 若发现既有缺陷、回归或规范不匹配：
   - 先修复；
   - 或将其作为前置任务插入 `TODO.md` 当前任务之前，并在 `PLAN.md` 记录阻塞原因；
   - 然后停止在本轮继续后续任务。
9. 完成后更新文档：
   - 在 `TODO.md` 标记任务完成；
   - 在 `PLAN.md` 反映当前状态与后续顺序；
   - 在本文件记录已完成步骤与任何计划调整。
10. 提交 Git 提交，提交信息对应本轮完成的任务，然后停止。

## 进度记录

- 已创建本计划文件，等待读取仓库状态与任务列表。
- 已检查最近一次提交：`edca42bac3b3d86999c84f9ce7f88664d1cf3ded`，提交说明仅为 `[T5000e2bR] Fix reachable owner-specialized MIR collection`，未发现仍需在本轮前插的额外待修复问题说明。
- 已读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务为 `T5000e2c`：
  - 目标是让 build / single-file LLVM frontend 切换到 MIR instance collection；
  - 同时收口 HIR eager materialization 主路径；
  - 并修复主路径对“相同 type args、不同 effect rows”实例身份坍缩的问题。

## 当前判断

- 现状：
  - `crates/scoop/src/commands/build.rs::lower_main_hir_for_build(...)` 仍直接调用
    `hir::lower_for_compilation_unit_multi_files_with_type_env(...)`，把
    `front.monomorph_keys` 直接交给 HIR lowering；
  - `crates/scoopc/src/llvm/frontend.rs::prepare_single_file_codegen_unit(...)` 也仍直接走同一条
    HIR eager materialization 路径；
  - `hir/lower` 中 `collect_generic_fun_instantiations(...)` / `collect_generic_member_fun_instantiations(...)`
    仍承担实例发现职责，其中前者忽略 `eff_args`，后者还会扫描 `TypeStore` 做 owner/member eager 发现。
- 结论：
  - `T5000e2c` 可在本轮直接实现，不必再拆子任务；
  - 但不能继续沿用“`MonomorphKey` 直接喂给 HIR eager materialization”主路径，否则无法满足
    effect-row 实例身份与 owner-specialized instance discovery 的边界要求。

## 细化实现方案

1. 在 `scoopc` 中新增“编译单元经 MIR instance collection 生成 LLVM 兼容 HIR 输入”的统一入口。
   - 该入口内部先调用既有 MIR materialization；
   - 再根据 MIR 产出的 `InstanceKey` 集合生成现有 LLVM codegen 仍需要的 monomorphic HIR 兼容输入。
2. 为 HIR lowering 增加“显式实例集合驱动”的兼容路径。
   - 不能再让 HIR 自己扫描 `TypeStore` / `MonomorphKey` 做主发现；
   - 需要支持从 `InstanceKey` 精确回查 AST generic 声明；
   - 需要支持 effect-row 具体绑定，而不是仅放占位符；
   - 兼容输出仍保持 `LoweredHir`，避免本轮提前重写 LLVM codegen。
3. 切换调用面：
   - `crates/scoop/src/commands/build.rs` 改为消费新的统一入口；
   - `crates/scoopc/src/llvm/frontend.rs` 改为消费同一入口，确保 single-file LLVM frontend 同步切换。
4. 增加回归测试。
   - 锁定 build/frontend 主路径不再坍缩 `wrap<Int, eff Boom>` / `wrap<Int, eff Zap>`；
   - 锁定 single-file LLVM frontend 也消费同一 MIR instance collection 主线；
   - 视需要补充 owner-specialized member/getter 在兼容 HIR 路径上的覆盖。
5. 全量整理与验证：
   - 更新 `TODO.md`、`PLAN.md`、本文件；
   - 运行 `cargo fmt --all`、相关定向测试、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`；
   - 提交本轮变更并停止。
# 2026-04-26 本轮执行计划（补充）

说明：这里记录的是可审计的执行计划、判断依据和进度摘要，不包含不可审计的内部推理细节。

## 当前目标

- 按 `TODO.md` 顺序完成首个未完成任务 `T5000e2c`，完成后立即停止。
- 在进入任务收尾前，先核对最新提交是否声明了需要先修复的既有问题；如果发现，先处理该问题。

## 已知状态

- 代码实现主变更已经落地：`build` 与单文件 LLVM frontend 已改为先走 MIR instance collection，再生成 LLVM 当前仍依赖的 HIR 兼容输入。
- 已完成定向验证：
  - `cargo fmt --all`
  - `cargo check --all-targets`
  - `cargo test -p scoopc single_file_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`
  - `cargo test -p scoop build_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`
- 尚待确认：
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 尚未完成的收尾动作：
  - 更新 `TODO.md`
  - 更新 `PLAN.md`
  - 继续补记本文件
  - 提交 git commit

## 本轮步骤

1. 查看最新提交信息，确认是否带有必须先修复的既有问题说明。
2. 核对 `TODO.md` 与 `PLAN.md` 当前状态，确认首个未完成任务仍为 `T5000e2c`。
3. 检查/等待全量验证结果；若已有会话不可用，则重新运行相关命令。
4. 若验证暴露既有缺陷，先修复缺陷并补充测试，再重新验证。
5. 若验证通过，更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md`，将 `T5000e2c` 标记完成并记录实际边界。
6. 以 `[T5000e2c] Route build frontend through MIR instance collection` 为主题提交本轮变更。
7. 停止，不进入下一条任务。

## 当前进度（执行中）

- 已核对最新提交：`[T5000e2bR] Fix reachable owner-specialized MIR collection`，提交正文未声明新的必须先修复事项。
- 已再次确认 `TODO.md` 的首个未完成任务仍是 `T5000e2c`。
- 全量验证现状：
  - `cargo test --all` 失败。
    - 失败集中在 7 个 LLVM / async / task 相关测试，统一症状为 `UnsupportedMainBody { kind: "call return type" }`。
    - 说明 build / single-file frontend 切换到新的 MIR instance collection 主路径后，仍有一类 effectful / task 入口形状没有被兼容 lowering 正确保住。
  - `cargo clippy --all-targets -- -D warnings` 失败。
    - 当前是 `crates/scoopc/src/hir/lower/mod.rs` 中 12 处 `needless_borrow`，属于本轮改动引入的直接收尾问题。
- 下一步：
  1. 先修掉 `clippy` 告警，避免噪音；
  2. 再最小复现并定位 `UnsupportedMainBody { kind: "call return type" }` 的来源；
  3. 修复该回归后重新跑定向测试、全量测试与 `clippy`。

## 已完成的关键修复

- 已修复 MIR instance collection 对非泛型 request root 的 reachable 扫描缺口：
  - 先前只扫描 direct-call，不会继续扫描 request root 创建出来的 closure body；
  - 现已在 `crates/scoopc/src/mir/materialize.rs` 中让 non-generic reachable scan 递归跟进 `Rvalue::MakeClosure { fn_ptr, .. }` 对应的 callable body；
  - 同时为所有带 body 的 MIR fun 建立按 FQN 的补充索引，使 async lowering 生成的匿名 lambda 也能被继续扫描。
- 已修复 generic MIR lowering 对占位式 effect terminator 的后续语句截断：
  - 先前 `lower_block_as_stmt` / `lower_block_as_expr` 一旦遇到 placeholder `Handle` / `Perform` terminator 就直接停止，导致 async body 中 `await` 之后的 `println(...)`、`__task_step_ready(...)` 等 direct call 根本不会进入 generic MIR；
  - 现已在 `crates/scoopc/src/mir/lower.rs` 中加入“仅对占位式 `Handle` / `Perform` 允许继续”的 continuation block 逻辑，把后续语句落到新的孤立 block 中保形，供 materializer 继续发现 direct-call 实例。
- 已补正式回归测试：
  - `crates/scoopc/src/mir/materialize.rs`：
    - `typechecked_compilation_unit_materialization_reaches_generic_calls_through_non_generic_async_closure_body`
  - `crates/scoopc/src/llvm/tests.rs`：
    - `single_file_frontend_reaches_async_task_helper_instances_through_perform_continuations`
- 已修复 `crates/scoopc/src/hir/lower/mod.rs` 中 12 处 `clippy::needless_borrow`。

## 当前验证状态

- 定向验证已通过：
  - `cargo fmt --all`
  - `cargo test -p scoopc single_file_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`
  - `cargo test -p scoopc single_file_frontend_reaches_async_task_helper_instances_through_perform_continuations -- --nocapture`
  - `cargo test -p scoopc typechecked_compilation_unit_materialization_reaches_generic_calls_through_non_generic_async_closure_body -- --nocapture`
  - `cargo test -p scoopc async_task_resume_ir_does_not_replay_original_await_site -- --nocapture`
  - `cargo test -p scoopc async_task_resume_replay_ir_terminates_step_fn_on_active_effect -- --nocapture`
  - `cargo test -p scoopc async_task_ir_uses_ordinary_scoop_task_helpers_not_legacy_runtime_abi -- --nocapture`
  - `cargo test -p scoopc single_file_minimal_ir_supports_handled_async_await -- --nocapture`
  - `cargo test -p scoopc task_step_ir_uses_ordinary_scoop_definition_not_legacy_poll_abi -- --nocapture`
  - `cargo test -p scoopc task_step_ir_uses_seqcst_atomic_claim_and_trap_without_mutex -- --nocapture`
  - `cargo test -p scoopc thread_join_statepoint_preserves_live_gc_locals -- --nocapture`
- 待完成的最终验证：
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

## 最终状态（待提交）

- 全量验证已完成并通过：
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 文档同步已完成：
  - `TODO.md` 已将 `T5000e2c` 标记为完成，并记录 build/frontend 切主线时顺带修掉的两个真实前置缺口；
  - `PLAN.md` 已补记 `T5000e2c` 的实现结果、回归测试与验证结论；
  - 本文件已补完执行轨迹与最终验证状态。
- 剩余动作：
  1. 检查工作区 diff 与 git 状态；
  2. 用 `[T5000e2c] Route build frontend through MIR instance collection` 提交；
  3. 停止，不进入 `T5000e2cR`。
