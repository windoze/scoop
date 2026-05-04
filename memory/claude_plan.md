## 当前执行计划

1. 读取 `TODO.md` 作为索引，并按引用顺序检查对应的 `TODO-Px.md`，定位首个标题未带 `[DONE]` 的详细任务。
2. 检查最近一次提交是否存在与该任务直接相关且明确未完成的问题；若存在，将其并入当前任务或作为前置依赖处理。
3. 阅读当前任务涉及的代码、测试、规范与约束，确认实现边界与验证要求。
4. 直接完成该任务；若遇到阻塞当前任务的真实缺口或规格不匹配，则先修复阻塞，或在对应 `TODO-Px.md` 中插入最小必要前置任务并同步 `TODO.md`。
5. 运行与该任务相关的验证，包括必要的测试、`cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`，并修复发现的问题。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
7. 在对应 `TODO-Px.md` 中将完成的任务标题标记为 `[DONE]`，补全完成记录；若索引发生变化，同步更新 `TODO.md`。仅当阶段计划本身变化时更新 `PLAN.md`。
8. 按要求创建一次 git 提交，然后停止，不继续下一个任务。

## 进展记录

- 已写入初始执行计划，下一步开始读取任务索引并定位首个未完成详细任务。
- 已确认 `TODO.md` 中首个未完成详细任务是 `TODO-P6-part2.md` 的 `P6-T03`。
- 已检查最近提交：`[P6-T02p] Publish callable version selection contracts`，它与 `P6-T03` 直接相关且已完成，无需另加未完成前置项。
- 已确认当前实现状态：`crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 仍为空壳；`crates/scoopc/src/llvm/emit.rs` 仍通过 `ensure_refactor_effect_lowering_is_supported(...)` 和 `ensure_refactor_reachable_callables_do_not_require_legacy_effect_body_lowering(...)` 对 effectful callable fail-fast。下一步需要审查现有 `codegen`/ABI helper 是否足以直接落地 `P6-T03`，若不足则判断是否存在新的、尚未记录的 blocker。
- 已进一步确认可直接实现的关键事实：
  - `mir_body.rs` 已有可复用的 straight-line lowering helper：`create_mir_local_slots`、`codegen_mir_statement`、`codegen_mir_operand(_expected)`、`load_mir_local`、`mir_local_slot`、`cg_ty_of_mir_type` 等；
  - `effect_refactor/{layout,types}.rs` 已发布 `Step`、frame、continuation object、surface-resume、owner trampoline、boundary operand、local runtime-error、handle dispatch、callable version、dynamic invoke 等 query；
  - 真实 `dump-effect-lowered` 已显示当前目标 fixture 的 state graph / boundary / handle contract 足够支撑 body emitter；暂未发现新的、必须先插入 TODO 的 blocker。
- 更新后的实施方案：
  1. 在 `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 实现 refactor body emitter，先提供：frame/continuation/source-ABI/Step 构造 helper、callable direct/dynamic entry lowering、boundary lowering、handle routing、surface-resume/owner-trampoline/internal-resume wrapper。
  2. 在 `crates/scoopc/src/llvm/emit.rs` 接入该 emitter：
     - 保留 ABI materialization；
     - 移除当前对 effectful callable 的 fail-fast；
     - 跳过这些 callable 的 legacy body lowering；
     - 让 C ABI `main` wrapper 改为通过 refactor entry 取结果，而不再内联 lower HIR body。
  3. 如实现过程中确实需要额外 query/iterator，再最小化补充 `effect_refactor/types.rs` 的访问面。
  4. 完成后补齐 `P6-T03` 的定向测试/fixture，运行任务要求的验证命令，再回写 TODO 与提交。
- 2026-05-04 最新结论：在真正对接 `Resume` boundary body lowering 时确认出现新的 contract blocker，已按要求停止实现并改为插入前置任务：
  - blocker：boundary-local `Resume` wrapper schema 与 runtime continuation object 实际 surface schema 可以不一致，但当前 handoff 没有 authoritative bridge；
  - 证据：`effect_multi_escape_indirect_direct_while.scoop` 的 dump 中 handle binder 发布 `k3`，而后续 resume boundary site25/site30/site35/site40 只发布 `k5`；`k5` 的 published dispatch 为 `ResumeBoundaryOnly`，没有 object-side method target；
  - 结论：若继续实现 `P6-T03`，backend 必须回 continuation local/source type/object shape 猜真实 route，违反 contract-first 约束；
  - 已执行：在 `TODO-P6-part2.md` 新增前置任务 `P6-T02q`，并同步 `TODO.md` 索引；`PLAN.md` 暂无需改动。
