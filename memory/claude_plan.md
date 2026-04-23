# 当前任务：T4016T7a 收口

更新时间：2026-04-24

## 思路摘要

当前目标不是继续推进下一个任务，而是把 `TODO.md` 中首个未完成任务 `T4016T7a` 完整收口，然后停止。

根据上一轮工作交接，主线实现已经基本完成，且大部分定向验证与 `cargo run -p scoop -- test` 已经通过。当前剩余的收尾问题只有一个：

- `cargo test --all` 里还有 1 个失败测试：
  - `llvm::codegen::effect::state_machine_emitter::tests::async_task_resume_replay_ir_terminates_step_fn_on_active_effect`

这个失败目前看起来不是新的生产逻辑错误，而是测试对 LLVM IR 中 SSA 临时变量名写死，导致在代码生成结构调整后发生编号漂移。修复方向应当优先是：

- 保持生产行为不变；
- 把该测试改成检查语义稳定的 IR 片段；
- 然后重新跑全量测试与 clippy；
- 如果全部通过，再更新 `TODO.md`、`PLAN.md`，提交一次 commit 并停止。

同时需要继续遵守：

- 如果在测试过程中发现新的既有 bug / regression / spec mismatch，必须立即转为当前优先级问题处理，不能绕过；
- 只能完成这一个任务，不能顺手做下一个；
- 所有文件编辑必须使用 `apply_patch`；
- 需要持续更新本文件，记录关键进展与计划变更。

## 已知代码状态

交接里已完成并应当保留的改动包括：

1. LLVM ordinary/internal call 的 aggregate 参数与 aggregate 返回值在含 GC refs 场景下改为 ABI 安全传递。
2. `gc-leaf-function` 误判已修复。
3. object/global init helper 已标注 `gc "statepoint-example"`。
4. conservative spill/writeback 只回写 stack-backed slot，不再错误写回 heap-backed frame/object field。
5. effect 相关 runtime 调用已切换到保 root 的调用路径。
6. runtime 已修复：
   - `InNative` 线程 roots 不完整；
   - `yield()` 不是 safepoint；
   - collect 入口遇到其他线程已发起 STW 时会傻等。
7. operator overload 调用路径已复用通用 top-level call lowering，避免 aggregate return/arg 的分叉实现再次失配。
8. 新增两个 runtime_gc fixture，用于覆盖 ordinary call aggregate transport 与 task step aggregate transport。

## 当前判断

从交接信息看，`T4016T7a` 的主实现已经完成，当前更像是“验证收口时暴露出的脆弱测试”。

因此本轮的最小闭环应当是：

1. 检查最新提交和 `TODO.md`/`PLAN.md` 当前状态，确认首个未完成任务仍是 `T4016T7a`。
2. 打开失败测试所在代码，确认断言确实依赖固定 SSA 名字，而不是掩盖新的行为回归。
3. 修改测试断言，使其验证：
   - active effect 分支会终止 step function；
   - continue 分支会把 replay 结果通过 write barrier 写入 resume slot；
   - 然后跳回 `site0_resume_merge`。
4. 跑定向测试确认这个测试通过。
5. 跑 `cargo test --all`。
6. 跑 `cargo clippy --all-targets -- -D warnings`。
7. 若全部通过：
   - 更新 `TODO.md`，把 `T4016T7a` 标记完成；
   - 更新 `PLAN.md`，同步当前状态；
   - 更新本文件，记录验证结果与完成状态；
   - 提交 commit；
   - 停止。

## 关键风险

1. 失败测试如果并非纯粹 SSA 编号漂移，而是 replay/store 路径结构真的变了，就必须先定位生产逻辑是否倒退，不能只“放宽断言”。
2. `cargo test --all` 或 `clippy` 可能再暴露新的既有问题；若出现，必须先修这些问题，或者把阻塞关系前插回 `TODO.md`/`PLAN.md` 后停止。
3. 工作树里已经有较多未提交改动，不能误回滚已有有效修复。

## 执行步骤

1. 查看最新提交信息，确认是否存在需要先处理的“提交中提到的既有问题”。
2. 查看 `TODO.md` 与 `PLAN.md`，确认 `T4016T7a` 仍是首个未完成任务，且当前无需拆分为新的子任务。
3. 打开失败测试与相关 IR 断言，检查当前失败点。
4. 修改测试为语义稳健断言。
5. 运行定向测试：
   - `cargo test -p scoopc async_task_resume_replay_ir_terminates_step_fn_on_active_effect --lib -- --nocapture`
6. 运行全量验证：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
7. 若验证通过，更新：
   - `TODO.md`
   - `PLAN.md`
   - `memory/claude_plan.md`
8. 提交：
   - 预期 commit message：`[T4016T7a] Fix GC-safe aggregate ordinary-call transport`
9. 停止，不继续执行下一个任务。

## 进展记录

- 2026-04-24：基于交接信息重建本轮计划，判断当前主要剩余项是 1 个脆弱 LLVM IR 测试失败，优先做测试收口与全量验证。
- 2026-04-24：已确认失败测试的生产 IR 语义仍正确，失败原因是断言把 `direct_call_effect_continue` 中的 replay 结果 SSA 名字写死为 `%call40`。已改为使用 `find_block_ir(...)` 提取 continue block，并检查 `resume_slot_`、`@scoop_gc_write_barrier`、`ptr addrspace(1) %call` 与回跳 `site0_resume_merge` 这些稳定语义片段。
- 2026-04-24：`cargo clippy --all-targets -- -D warnings` 暴露出 3 个由当前改动引入的质量问题，已通过局部重构收口：
  - 删除 `collect_gc_ptr_leaf_slots_in_spill()` 里只用于递归透传的无效 `Span` 参数；
  - 为 `codegen_bound_call_args()` 引入 `BoundCallArgsSpec`；
  - 为 `bind_ordinary_param_local()` 引入 `OrdinaryParamLocalBinding`。
- 2026-04-24：在重跑 `cargo test --all` 时发现 `gc_immix_compaction` test binary 会卡死。定位后确认是既有测试隔离问题：
  - 同一 test binary 内有两个依赖全局 GC/runtime 状态的 Immix compaction 测试；
  - Rust test harness 会并发执行它们；
  - 其中一个测试在 `InNative` 期间发起 STW 时，另一个测试线程仍处于 `Running`，导致死锁。
  已在 `crates/scoop_runtime/tests/gc_immix_compaction.rs` 中补上进程内 `Mutex` 串行化保护，和已有 `gc_immix_allocator.rs` 的约束保持一致。下一步重新跑该文件与全量验证。
- 2026-04-24：最终验证已全部通过：
  - `cargo test -p scoopc async_task_resume_replay_ir_terminates_step_fn_on_active_effect --lib -- --nocapture`
  - `cargo test -p scoop_runtime --test gc_immix_compaction -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`（`fixtures: ok (1168)`）
- 2026-04-24：`T4016T7a` 已收口，`TODO.md` / `PLAN.md` 的当前队首应切换为 `T4016T8`。下一步只需整理工作树、提交 `[T4016T7a] Fix GC-safe aggregate ordinary-call transport`，然后停止。
