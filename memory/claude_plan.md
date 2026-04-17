# 执行计划

## 目标

本次调用只完成 `TODO.md` 中第一个未完成任务；若存在来自最新提交提到的遗留问题，则先修复这些问题，再处理任务。完成后需要更新 `TODO.md` 与 `PLAN.md`、执行相关测试、提交 git commit，然后停止。

## 约束与执行原则

- 先检查最新提交信息，确认是否提到需要先处理的遗留问题。
- 读取 `TODO.md`，识别第一个未完成任务。
- 如果该任务过大或被前置缺陷阻塞，则把它拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
- 不接受规避方案；若发现与规范不符的实现缺口，必须把该缺口前置为任务并调整依赖顺序。
- 任何关键进展、计划变更、阻塞原因与验证结果，都要及时追加到本文件。
- 本次只完成一个任务或一个新拆分出来的首个子任务，完成后提交并停止。

## 初始步骤

1. 查看最新一次 git commit 的标题与正文，确认是否提到已知问题或待修复事项。
2. 打开 `TODO.md` 与 `PLAN.md`，确认当前任务顺序和计划状态。
3. 判断第一个未完成任务是否可在本轮完整落地。
4. 若可落地，阅读相关代码与测试，实施修改。
5. 运行与该任务相关的测试；若修改影响较广，则补充运行更全面的检查，包括必要时的 `cargo test`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 等。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态、验证结果与任何计划调整。
7. 提交一次清晰的 git commit，并停止。

## 当前状态

- 已创建本计划文件。
- 已检查最新提交、`TODO.md`、`PLAN.md`。
- 最新提交 message 未提到新的遗留问题；当前首个未完成任务为 `T3013`。
- 已复现 `T3013` 相关现状：
  - `handle_compound_result.scoop` 失败，错误为 `u64 word from composite value`。
  - `effect_nonresuming_payload_struct_indirect.scoop` 失败，错误为 `u64 word from composite value`。
  - `continuation_resume_continuation.scoop` 已通过，说明现有 `resume_gc_ref` 通道对 GC 引用类型已工作。
- 初步判断：当前阻塞是统一 transport 合同缺少“复合值 carrier”，不是三个彼此无关的独立问题。

## 进展记录

### 2026-04-17：任务定位

1. 查看最新 git commit，未发现 commit message 中声明的新遗留问题。
2. 检查 `TODO.md` / `PLAN.md`，确认首个未完成任务是 `T3013`。
3. 读取 `effect/mod.rs`、`effect/state_machine_emitter.rs`、`llvm/codegen/mod.rs`、`gc.rs`、`ty.rs`，定位到当前 transport 断点集中在：
   - `coerce_u64_word`
   - `write_resume_payload_to_continuation`
   - `store_result_to_frame`
   - `read_result_from_frame`
   - `read_binder_from_perform_slot`
   - `emit_perform_op` / `codegen_perform_expr`
4. 初步实现策略：
   - 复用现有 typed alloc + type descriptor 机制，为 tuple/struct/rich enum 等值类型复合值提供统一的 GC-managed box。
   - 统一新增 boxing/unboxing helper，并让 perform binder、handle result、resume payload 三条路径共享同一套 helper。
   - 优先保持现有 scalar / direct GC ref 路径不变，只为非 word 且非直接 GC ref 的值类型走 box + `*_gc_ref` 通道。

### 2026-04-17：当前待执行

1. 继续阅读现有 boxing / type descriptor / LLVM 类型命名辅助代码，设计复合值 transport helper 的落点与命名方案。
2. 实现共享 composite transport helper。
3. 把三条路径切换到共享 helper。
4. 补单测 / 运行定向 fixture，并根据结果决定是否需要调整 `TODO.md` / `PLAN.md` 的任务边界。

## 更新规则

- 每完成一个关键步骤后，补充“进展记录”小节。
- 若任务拆分或依赖顺序变更，记录原因、受影响任务与新的执行顺序。
- 若发现阻塞缺陷，记录缺陷现象、影响范围、对应新增任务以及本轮停止原因。
# 当前回合计划（2026-04-17）

## 约束与目标

- 本回合只处理 `TODO.md` 中第一个未完成任务。
- 在继续任何实现前，先把当前判断、执行顺序和后续更新规则记录到本文件。
- 不接受 workaround；如果发现更前置的真实缺陷阻塞当前任务，必须先调整 `TODO.md` / `PLAN.md`，提交后停止。

## 当前判断

- 最新 commit `"[T3012R] Review unified expected-context closure alignment"` 本身没有在 commit message 中声明额外遗留问题。
- 目前首个未完成任务仍是 `T3013`：扩展 effect payload / handle result / resume payload transport，覆盖 composite 值。
- 之前已经实现 transport 统一层，并让 perform / handle result / continuation resume 三条路径共享。
- 当前最关键的剩余问题不是最初的 transport 崩溃，而是回归与缺口：
  - `tests/fixtures/run-pass/continuation_resume_ref_class.scoop` 在全量 suite 中是当前最早失败点，表现为第一次 `resume` 后执行流提前中断，stdout 少了后半段。
  - `tests/fixtures/run-pass/continuation_resume_enum.scoop` 仍存在 rich enum 相关 coercion 失败。
- 因此，本回合首先要确认 `continuation_resume_ref_class.scoop` 是否属于 `T3013` transport 改动引入/暴露的问题；如果是，就修到通过。然后继续推进到 enum 剩余缺口，直到 `T3013` 可被完整关闭，或者确认存在更前置依赖需要重排任务。

## 执行步骤

1. 检查工作树与当前未提交改动，确认摘要中提到的文件和调试残留状态。
2. 读取 `TODO.md` / `PLAN.md` / 本文件，确认 `T3013` 仍是首个未完成任务且尚未被拆分。
3. 复现当前最早失败点：
   - 先跑 `continuation_resume_ref_class.scoop`
   - 必要时查看对应源码、golden、相关 codegen/effect 状态机实现
4. 定位并修复 `Continuation<Box>` / ref-class payload / 多次 `resume` 场景下的执行流问题。
5. 重新运行相关 fixture，并继续跑全量 `cargo run -p scoop --features llvm -- test`，确认新的最早失败点。
6. 如果新的最早失败点仍在 `T3013` 范围内（当前高概率是 `continuation_resume_enum.scoop`），继续修复 rich enum transport/coercion 剩余缺口。
7. 在 `T3013` 完成后，补跑质量门槛：
   - 相关 fixture
   - `cargo run -p scoop --features llvm -- test`
   - `cargo clippy --all-targets -- -D warnings`
8. 更新文档状态：
   - `TODO.md` 标记任务完成，或如被阻塞则按依赖重排
   - `PLAN.md` 同步当前状态
   - 本文件记录关键进展与计划变化
9. 提交一次 git commit，然后停止，不继续下一个任务。

## 进度更新规则

- 每完成一个关键节点，就同步更新本文件：
  - 已复现的问题与现象
  - 发现的根因
  - 已做的代码修改
  - 已跑的测试及结果
  - 是否需要重排 `TODO.md`

## 2026-04-17 关键进展：T3013 边界校正

- 已复现 `tests/fixtures/run-pass/continuation_resume_ref_class.scoop`，当前实际输出只到：
  - `after_handle`
  - `got1`
  - `alpha`
  - `10`
- 进一步对比 `effect_escape_continuation_multi_perform_while_loop.scoop` 与 `effect_escape_continuation_gc_stress_multi_string.scoop` 后确认，这不是 `Box`/GC-ref payload 的偶发问题，而是更普遍的 escaped continuation 多次 `perform` 缺口：
  - 第一次 `resume(...)` 会正确回到 resumed body，并执行到下一次 `perform`
  - 但 resumed continuation 在下一次 `perform` 后不会重新进入 captured handler 的 dispatch loop，caller-tail 直接被截断
- 因此，当前 `T3013` 已实现的共享 transport helper 解决了：
  - perform binder 的 composite transport
  - handle result 的 composite transport
  - continuation resume payload 的 shared boxing/unboxing transport
- 但 `continuation_resume_ref_class.scoop` 暴露的是已存在的 escaped continuation handler-context / multi-perform 恢复语义缺口，更接近 `T3015` 的问题域，而不是新的 transport 根因。
- 接下来的执行决策：
  1. 恢复 `continuation_resume_ref_class.scoop` 的 xfail，并把原因改成真实、具体的 `T3015` 语义缺口说明。
  2. 更新 `TODO.md` / `PLAN.md`，记录 `T3013` 已收口到共享 transport 合同，而 `ref_class` / multi-perform resumed continuation 问题留在 `T3015`。
  3. 按修正后的边界重新跑 `T3013` 定向验证、workspace tests、clippy，并决定是否可以关闭 `T3013`。

## 2026-04-17 验证结果与收口结论

- 已恢复 `tests/fixtures/run-pass/continuation_resume_ref_class.scoop` 的 xfail，并把原因改成 `T3015` 的真实 handler-context / redispatch 缺口说明。
- `T3013` 定向验证已通过：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/handle_compound_result.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_nonresuming_payload_struct_indirect.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_struct.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_tuple.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop`
- 边界验证结果：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_enum.scoop` 现在报的是 `value coercion`，不再是 `u64 word from composite value`，说明共享 transport 合同已接通，剩余问题留给 richer enum escaped-continuation 路径。
  - `cargo run -p scoop --features llvm -- test` 的首个停止点是 `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop` “期望失败，但执行成功”，属于 `T3017` stale xfail cleanup，不是 transport 回归。
- 质量门槛：
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 结论：
  - 当前可以将 `T3013` 收口为完成。
  - 下一项应推进 `T3013R`。
