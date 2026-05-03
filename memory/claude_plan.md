# 执行计划

说明：我不会记录或暴露不可公开的内部推理，但会在此维护可检查的执行计划、关键判断、进度与阻塞信息。

1. 读取 `TODO.md`，把它当作任务索引而不是详细规范。
2. 按索引顺序读取对应的 `TODO-Px.md`，定位首个标题未标记 `[DONE]` 的详细任务。
3. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；若是，则将其视为当前任务的一部分或必要前置。
4. 阅读当前任务涉及的代码、测试、规范与依赖范围，确认需要修改的最小文件集合。
5. 实现当前任务；若发现阻塞当前任务且必须先修复的真实缺陷或缺失能力，则先在对应 `TODO-Px.md`/`TODO.md` 中加入最小前置任务并停止在该前置变更处。
6. 运行与当前任务直接相关的验证命令；若任务完成，再补充运行要求中的质量命令（至少包含相关测试，并尽量满足 `cargo clippy --all-targets -- -D warnings`）。
7. 更新进度文档：
   - 在对应 `TODO-Px.md` 中把当前任务标题前缀改为 `[DONE]`，并补充完成记录。
   - 如任务索引需要同步，则更新 `TODO.md`。
   - 仅当阶段级计划发生变化时才更新 `PLAN.md`。
8. 提交本次变更，提交信息使用当前任务号。
9. 停止，不进入下一个任务。

## 进度日志

- 2026-05-03（本轮）：已重新核对 `TODO.md` 与 `TODO-P6.md`，确认当前首个未完成详细任务为 `P6-T02k`：发布 `HandleDispatch` arm payload binder / escape-continuation binder contract。
- 2026-05-03（本轮）：已核对最近一次提交为 `[P6-T02k] Track handle-arm binding prerequisite`；它直接对应当前任务的 blocker 记录，因此本轮按该前置任务本身继续执行，而不是额外插入新的前置项。
- 2026-05-03（本轮）执行计划：
  1. 阅读 late-lowered IR、frame schema、dump surface、以及 LLVM effect_refactor ABI/query 代码，确认现有 `HandleDispatch`/binder 发布缺口的最小改动面。
  2. 为 handle arm payload binder 与 escape continuation binder 增加 authoritative handoff 结构与查询接口，并让 dump surface 可见。
  3. 在 LLVM ABI/query 层接入 fail-fast 校验，确保 `P6-T03` 后续可以只依赖 published contract，而不回 canonical MIR handle arm 恢复绑定形状。
  4. 补充/更新定向测试与 `dump-effect-lowered` 验证。
  5. 若任务完成，则把 `P6-T02k` 标记为 `[DONE]`，同步 `TODO.md`，记录完成信息并提交；若发现新的真实前置缺口，则只记录最小前置任务并停止。
- 2026-05-03（本轮）已完成实现：
  - `LateLoweredHandleDispatchContract` 现已发布 per-arm payload binder / continuation binder contract，包含 arm ordinal、payload tuple type、payload binder local+optional frame slot，以及 optional continuation binder 的 local+optional frame slot+continuation schema+continuation object。
  - `dump-effect-lowered` 已公开上述 contract。
  - `RefactorHandleDispatchLayout` / LLVM ABI query 现已发布对应的 per-arm binder layout，并对 canonical MIR arm metadata、payload binder 次序、continuation binder presence、以及 continuation surface-resume binding 做 fail-fast 校验。
  - 顺手修复了一个直接阻塞本任务验证的相关问题：HandleDispatch ABI materializer 现在只对实际含 `HandleDispatch` 的 callable 要求 handle system slots，不再错误拒绝纯 helper / 无 handle 的 callable。
- 2026-05-03（本轮）已完成验证：
  - `cargo test -p scoopc refactor_handle_arm_binding_contract`
  - `cargo test -p scoopc refactor_handle_arm_continuation_binding`
  - `cargo test -p scoopc refactor_handle_arm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 2026-05-03（本轮）下一步：回写 `TODO-P6.md` / `TODO.md` 的完成状态并提交。

- 已创建执行计划文件，下一步开始读取任务索引并定位首个未完成详细任务。
- 已确认首个未完成详细任务为 `TODO-P6.md` 中的 `P6-T03`：按 P5 state graph / boundary contract 完成 refactor LLVM body lowering。
- 最近一次提交为 `[P6-T02j] Publish handle-dispatch completion contract`，与当前任务直接相关，但未显示新的未完成 issue；因此继续按 `P6-T03` 执行。
- 当前执行子计划：
  1. 阅读 `effect_refactor` 现有 `types/layout/mod`、LLVM emit 入口、以及当前 body lowering/fail-fast 位置。
  2. 阅读 P5 late-lowered IR 中 `State`/`Boundary`/`HandleDispatch`/`LocalRuntimeError`/`impl_plan` 的 authoritative handoff。
  3. 判断是否仍存在阻塞 `P6-T03` 的未跟踪前置缺口；若有，按规则补到 `TODO-P6.md`/`TODO.md` 并停止。
  4. 若无阻塞，则在 `crates/scoopc/src/llvm/codegen/effect_refactor/` 下实现最小正确的 body emitter / verifier，并把 refactor LLVM stage 接到该实现。
  5. 补充任务要求的定向测试与 fixtures 验证。
  6. 更新 `TODO-P6.md`（必要时同步 `TODO.md`）、提交并停止。

- 已完成上下文核对：
  - `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 目前仍是空壳；
  - `crates/scoopc/src/llvm/emit.rs` 仍会对 effectful reachable callable 执行 fail-fast，而不是 lower；
  - P6-T02c ~ P6-T02j 已把 `resume`、dynamic invoke、runtime-error、synthetic source ABI、`HandleDispatch` 等 body lowering 所需 contract 发布到 authoritative handoff；
  - 暂未发现必须新增到 TODO 的未跟踪前置缺口，当前缺的是实现而不是 contract。
- 下一步实现重点：
  1. 新建 refactor body/codegen 入口，消费 late-lowered callable + ABI query + canonical MIR body。
  2. 用 state graph block/loop lower `Goto` / `Branch` / `Return` / `Suspend` / `HandleDispatch` / `LocalRuntimeError` / `Abandon`。
  3. 先把 refactor callable entry / continuation / state dispatch 接到 `emit.rs`，替换当前对 effectful callable 的 fail-fast 主路径。

- 关键阻塞判断（2026-05-03）：
  - 在真正落地 `HandleDispatch` arm entry lowering 前，发现当前 authoritative handoff 还没有发布 arm payload binder / escape continuation binder 绑定合同。
  - 现状问题：
    - `HandleDispatch` 只发布了 handled case -> arm state / completion-state / carrier；
    - `LateLoweredFrameSlotKind::HandleBinder { site_id, ordinal }` 只能说明“这个 handle site 有某些 payload binder slot”，无法把它们稳定绑定到具体 handled case / arm state；
    - continuation binder 没有等价 published contract；
    - 因而 `P6-T03` 若继续实现，只能回 canonical MIR `TerminatorKind::Handle { binder_locals, continuation_local, .. }` 或 HIR 现场恢复 arm entry shape。
  - 这与 `P6-T03` 明确禁止回 `mir::Body` / shape source 猜语义的约束直接冲突，因此判断为真实前置缺口，而不是可接受的实现细节。
- 已采取动作：
  1. 在 `TODO-P6.md` 中插入新的前置任务 `P6-T02k` 与 review 任务 `P6-T02kR`。
  2. 在 `TODO.md` 中同步新增索引条目。
  3. 在 `P6-T03` 的依赖与完成记录中记录该 blocker。
- 当前计划调整：
  - 本次 invocation 不再继续实现 `P6-T03` 代码；
  - 先提交前置任务重排与 blocker 记录，交由下一次 invocation 从 `P6-T02k` 开始继续。

- 2026-05-03（本轮，最新核对）：重新读取 `TODO.md` 与 `TODO-P6.md` 后，确认当前首个未完成详细任务并不是 `P6-T03`，而是 review 任务 `P6-T02kR`。
- 本轮 `P6-T02kR` 执行计划：
  1. 复核 `TODO-P6.md` 中 `P6-T02k` / `P6-T02kR` 的要求与完成条件，锁定 review 的判断标准。
  2. 阅读 `crates/scoopc/src/effect_lowered/{ir,materialize,dump}.rs` 与 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`，确认 handle arm payload binder / continuation binder contract 是否已 authoritative 发布到 late-lowered handoff 与 LLVM query。
  3. 搜索 `crates/scoopc/src/llvm/codegen/effect_refactor` 与 `crates/scoopc/src/effect_refactor_pipeline` 中是否仍把 canonical MIR `binder_locals` / `continuation_local` / `TerminatorKind::Handle` 当成 refactor LLVM 主实现的语义来源。
  4. 重新运行 `P6-T02kR` 要求的测试与命令，确认 dump surface、query 发布、以及 fail-fast 行为仍成立。
  5. 若 review 通过，则把 `P6-T02kR` 标记为 `[DONE]`、同步 `TODO.md`、更新完成记录并提交；若发现真实缺口，则按最小前置任务原则回写 `TODO-P6.md`/`TODO.md` 并停止。
- 2026-05-03（本轮，`P6-T02kR` 结论）：review 通过，未发现新的 blocker。
- 关键结论：
  1. `LateLoweredHandleDispatchContract` 已 authoritative 发布 per-arm `payload_binders` 与 optional `continuation_binder`；`dump-effect-lowered` 也公开了这层 surface。
  2. `RefactorHandleDispatchLayout` / `RefactorHandleArmLayout` 已把上述 binder contract 发布到 LLVM query，后续 `P6-T03` 可直接按 handled case 查询 arm payload/continuation 绑定。
  3. `llvm/codegen/effect_refactor/layout.rs` 中对 `binder_locals` / `continuation_local` / `TerminatorKind::Handle` 的命中仅用于 ABI materialization fail-fast 交叉校验 published contract 漂移；它没有把 canonical MIR arm locals 当成 query/export 的 authoritative source。
  4. `effect_refactor_pipeline` 中的相关命中仅位于 `mir_stage.rs`（contract 生成/测试侧），不在 refactor LLVM 主 lowering 入口。
  5. 已复跑 review 要求的两组定向测试、两条 `dump-effect-lowered` 命令与 `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`，均通过。
- 下一步：提交 `P6-T02kR` 完成记录与索引同步，然后停止，等待下一次 invocation 从 `P6-T03` 继续。
