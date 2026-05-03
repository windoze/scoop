# Claude Plan

## Notes
- 按用户要求，先记录高层执行计划与进度日志；此文件不会包含逐字内部思维，只保留可审阅的执行步骤、发现与决策。

## Initial Plan
1. 读取 `TODO.md`，把它当作索引使用。
2. 按索引顺序读取对应的 `TODO-Px.md` 详细任务文件，定位第一个标题未加 `[DONE]` 的任务。
3. 检查最近一次提交是否直接提到与该任务相关且未收尾的问题；若是，则将其视为任务的一部分或必要前置。
4. 阅读与当前任务直接相关的代码、测试、规范和任务约束，确认需要实现或修复的最小正确改动。
5. 实现当前任务；若遇到阻塞当前任务的真实前置缺陷，则在对应 `TODO-Px.md` 中最小化新增前置任务，并同步 `TODO.md`，然后停止。
6. 运行与该任务直接相关的测试与必要的质量检查；修复发现的问题直到通过，或在确有阻塞时按流程记录并停止。
7. 更新 `TODO-Px.md` 的完成记录并将任务标题标记为 `[DONE]`；若索引需要同步，则更新 `TODO.md`。
8. 仅在阶段级计划发生变化时更新 `PLAN.md`。
9. 提交当前变更，提交信息使用当前任务号。

## Progress Log
- 本次调用开始：先重新读取 `TODO.md` 作为索引，再按顺序检查相关 `TODO-Px.md`，确认当前首个未完成详细任务；若上一轮已插入新的前置任务，则以该前置任务为当前执行单元。
- 当前高层计划：确认任务 -> 阅读相关代码与最近提交 -> 实现或在必要时最小化补充前置任务 -> 跑定向测试与 `clippy` -> 更新 `TODO-Px.md`/`TODO.md`/必要时 `PLAN.md` -> 提交并停止。
- 已按 `TODO.md` 索引与 `TODO-P6.md` 详细任务顺序确认：当前首个未完成详细任务是 `P6-T02d`，不是 `P6-T03`；本次只处理 `P6-T02d`。
- 本次调用开始：先复核 `TODO.md`/`TODO-Px.md` 与当前工作树状态，确认上次记录的 `P6-T02c` 是否已经提交；若未提交，则把这批变更作为当前任务收尾并原子提交。
- 已创建本文件并记录初始计划，下一步开始读取任务索引与详细任务文件。
- 已读取 `TODO.md` 与 `TODO-P6.md`，确认索引里的首个未完成详细任务是 `P6-T02c`：发布 continuation surface-resume ABI/query contract。
- 已检查最近一次提交：`[P6-T02c] Track continuation surface-resume ABI blocker`。它直接说明 `P6-T03` 被当前任务阻塞，因此本次继续按 `P6-T02c` 执行，不做额外历史问题扫荡。
- 已完成首轮实现：
  - 在 `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs` 新增 `RefactorContinuationSurfaceResumeLayout`，把 source-visible `resume(...) -> Step_F` 的 LLVM 签名与 `ContinuationSchemaId` 绑定；
  - 在 `RefactorAbiQuery` 中新增 `surface_resume_layout(...)` 查询面；
  - 在 continuation object layout 中新增按 `ContinuationSchemaId` 发布的 surface-resume binding，显式记录该对象支持的 source-visible resume schema 与返回 `Step_F` schema；
  - 在 `layout.rs` 中新增 surface-resume layout materialization，以及对 resume site/continuation object 的 contract 校验与 fail-fast；
  - 已补充 `layout.rs` 定向单测，覆盖 resume-site 查询、缺失 contract 拒绝，以及 `Unit` payload 的 surface-resume ABI 退化。
- 下一步：运行 `cargo fmt`、定向 `cargo test`、`dump-effect-lowered` 与 `clippy`，根据结果继续修正。
- 首轮测试暴露真实缺口：`effect_resume_if_else_branch_single_perform.scoop` 的 `ResumeSiteEffectFacts` 使用 `continuation_schema=k3 / out_step_schema=s4`，但 continuation object 原始 `surface_resumes()` 只覆盖 owner callable outward step 的 `k0/k1`。这说明 source-visible `k.resume(...)` contract 不能只从 object 自带的 step-case shell 推出，还必须消费 owner callable 的 `Resume` boundary facts。
- 已据此调整实现：surface-resume layout 现在由两类 authoritative handoff 联合注册（object `surface_resumes()` + `ResumeSiteEffectFacts`）；continuation object 的 surface-resume binding 也同步覆盖 owner step-case schema 与 owner callable 的真实 resume-site schema。
- 已完成验证：
  - `cargo test -p scoopc refactor_llvm_surface_resume_layout`
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_llvm_unit_abi`
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 已同步 `TODO-P6.md` 与 `TODO.md`，将 `P6-T02c` 标记为完成。
- 本次调用复核后确认：`P6-T02c` 已由最新提交 `[P6-T02c] Publish continuation surface-resume ABI query` 落地，因此当前首个未完成详细任务是 `P6-T03`。
- 已复现 `P6-T03` 当前失败：`cargo run -p scoop -- --effect-pipeline refactor build tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop -o /tmp/effect_resume_if_else_branch_single_perform.out` 仍会在 stage 边界报 `RefactorEffectLoweringUnsupported`。
- 已进一步确认新的真实 blocker：`LateLoweredCallBoundaryLowering` 虽发布 `CallSiteTarget` / `CallTargetMode` / `invoke_args_tuple_ty` / callee `StepSchema`，但当前 `RefactorAbiQuery` 只按 callable version 暴露 static `dynamic_entry/direct_entry` 签名，没有 runtime callable value -> canonical dynamic `invoke(args_tuple) -> Step_F` 的 LLVM query/载体 contract。对 `CandidateSet` / `DynamicFallback` call boundary，继续实现 `P6-T03` 会被迫回 `CallKind::{Closure, FunValue, Virtual, Interface}` / legacy wrapper 现场重建 ABI，违背任务约束。
- 计划已调整：本次不继续硬做 `P6-T03`，而是在 `TODO-P6.md` / `TODO.md` 中最小化新增前置任务 `P6-T02d`，把 dynamic invoke ABI/query gap 显式化并让下一次调用先补这层 contract。

## Task Plan: P6-T02c
1. 检查当前工作树，确认是否存在未提交改动以及是否需要在结束时一并提交。
2. 阅读 `effect_facts`、`effect_lowered`、`llvm/codegen/effect_refactor/{types,layout}.rs`、以及可能承接 query 的调用点，找出当前 continuation surface `resume(...)` contract 缺失在哪里。
3. 设计并实现最小正确改动：
   - 为 `ContinuationSchemaId`（或等价稳定键）发布 surface-resume ABI/layout 查询对象；
   - 将 `resume_tuple_ty` / `answer_ty` / `out_step_schema` 与 continuation object/internal mapping 对齐；
   - 对缺失或漂移的 contract 增加结构化 fail-fast。
4. 补充/更新定向测试，覆盖：
   - ABI query 可从 authoritative continuation schema 解析到 surface-resume layout；
   - 缺失 contract 时显式拒绝；
   - 现有含 `resume(...)` 的定向路径可以通过新的 handoff 暴露所需信息。
5. 运行任务要求的测试与必要的 `clippy`，修复出现的问题。
6. 更新 `TODO-P6.md`、同步 `TODO.md`，记录完成情况与验证命令。
7. 提交本次所有未提交变更并停止。

## Task Plan: P6-T02d
1. 复核 `TODO-P6.md` / `TODO.md` 的未提交变更，确认它们确实只是把 `P6-T03` 的真实 blocker 显式化为新的前置任务 `P6-T02d`。
2. 阅读 `P6-T02d` / `P6-T03` 相关段落，以及 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`、late-lowering call-boundary facts、和当前 ABI query 调用点，定位 canonical dynamic-invoke callable-object contract 缺口。
3. 以最小正确改动补齐 callable object 的 canonical dynamic `invoke(args_tuple) -> Step_F` ABI/query contract，并在缺失 authoritative contract 时 fail fast，避免 `P6-T03` 在 backend 现场猜 indirect call 入口。
4. 补充/更新定向测试，覆盖 dynamic invoke ABI query 可解析、缺失 contract 显式拒绝、以及 build/dump 路径可消费这层 contract。
5. 运行任务要求的定向测试与 `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`，根据结果继续修正。
6. 将 `P6-T02d` 标记为 `[DONE]`，同步 `TODO.md`，并把本轮涉及的全部未提交文件一并提交。

## Progress Log (Current Invocation)
- 已复核 `TODO.md` 索引与 `TODO-P6.md` 明细：当前首个未完成详细任务仍是 `P6-T02d`，`P6-T03` 必须等待它完成后才能继续。
- 已检查最新提交：`[P6-T02c] Publish continuation surface-resume ABI query`；它与当前任务直接相邻，但未完成 `P6-T02d`。
- 已检查工作树：存在未提交变更 `TODO-P6.md`、`TODO.md`、`memory/claude_plan.md`。这与上轮在识别 `P6-T03` blocker 后新增 `P6-T02d` 但尚未提交的状态一致；本轮会先核对这些改动，再把实现、任务记录与这些未提交文件一起原子提交。
- 已完成首轮实现：
  - `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs` 新增按 call boundary 发布的 `RefactorDynamicInvokeLayout` / carrier layouts / `RefactorCallTargetQuery`，并让 `RefactorCallableEntryLayout` 记录 `invoke_args_tuple_ty`；
  - `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 现在基于 `LateLoweredBoundaryLowering::Call` + canonical MIR `CallKind` 物化 dynamic invoke contract，并对缺失 metadata / CandidateSet shell / contract drift fail fast；
  - `crates/scoopc/src/llvm/emit.rs` 已补上 ABI visibility pass-view handoff，修复 pure `main` build fixture 下不可达 helper dynamic-invoke contract 丢失；
  - 已新增 build fixture `tests/fixtures/build/effect_refactor_dynamic_invoke_candidate_set_emit_llvm.scoop`。
- 已完成验证：
  - `cargo test -p scoopc refactor_llvm_call_target_query`
  - `cargo test -p scoopc refactor_llvm_dynamic_invoke_query`
  - `cargo test -p scoopc refactor_llvm_callable_carrier_layout`
  - `cargo test -p scoopc refactor_llvm_unit_abi`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_candidate_set_emit_llvm.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 已同步 `TODO-P6.md` / `TODO.md`：将 `P6-T02d` 标记为完成并写入完成记录；`PLAN.md` 无需更新，因为阶段级顺序与退出条件没有改变。
- 下一步：检查最终 diff，确认只包含本任务需要提交的文件，然后使用 `P6-T02d` 提交信息原子提交并停止。
