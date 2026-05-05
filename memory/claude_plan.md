# 当前执行计划

## 目标

完成 `TODO.md` 索引指向的第一个未完成详细任务，并在完成或遇到必须前置处理的阻塞项后停止。

## 执行步骤

1. 读取 `TODO.md`，只把它作为任务索引使用。
2. 按索引顺序读取对应的 `TODO-Px.md` 文件，依据详细任务标题中的 `[DONE]` 标记确定第一个未完成任务。
3. 查看最近提交信息，判断是否明确提到与当前任务直接相关的未完成问题；若相关，将其作为当前任务的一部分或必要前置项处理。
4. 阅读当前任务的详细要求、约束、依赖和验证方式。
5. 检查相关代码和测试，定位最小正确实现范围。
6. 实施当前任务；若发现阻塞当前任务的规格缺口或实现边界，不绕过，改为在对应 `TODO-Px.md` 中插入最小必要前置任务并同步 `TODO.md`，然后提交并停止。
7. 运行与任务相关的测试；如有失败，优先修复由本任务引入或阻塞本任务完成的问题。
8. 完成后更新详细任务标题为 `[DONE]`，填写完成记录，并同步 `TODO.md` 中同一任务的 `[DONE]` 状态。
9. 必要时更新本文件记录关键进展；仅当阶段级计划变化时才更新 `PLAN.md`。
10. 按要求创建 Git 提交，提交本轮完成任务涉及的所有变更。
11. 停止，不继续下一个任务。

## 当前状态

- 已定位第一个未完成详细任务：`TODO-P7.md` 中的 `P7-T03`。
- 最近提交为 `P7-T02X` 修复项，未明确声明与 `P7-T03` 直接相关的未完成问题；`P7-T03` 的完成记录中此前阻塞项已通过 `P7-T02U/W/X` 等前置任务处理。
- `cargo test --all` 已通过。
- `cargo run -p scoop -- test` 首轮失败在 `effect_refactor_direct_handle_resume_emit_llvm.scoop`：resume-boundary wrapper projection outward continuation schema 未发布到 surface-resume inventory，导致 LLVM body lowering 缺少 k2 surface ABI。
- 已修复：wrapper projection 的 outward continuation schemas 现在按同一 resume boundary owner route 发布 inventory，并带上相同 wrapper projection；新增定向单测已通过，失败 fixture 已通过。
- 重跑 `cargo run -p scoop -- test` 后出现 `continuation_resume_runtime_error_boundary.effectlowered` golden 差异；该差异来自新增 surface-resume inventory/continuation composition contract，已同步 golden，并且该 fixture 已通过。
- 另一个 effect-lowered fixture `dispatch_and_resume_call.scoop` 因同一 contract 扩展产生 golden 差异，已同步并通过。
- `continuation_resume_answer_replay_basic.scoop` 暴露 nested handle routing 歧义：同一 boundary/case 同时命中内外层 handlers。已调整 LLVM body emitter 按 handle 嵌套深度选择最近的非 `emit_outward` routing；该 run-pass fixture 已通过。
- `continuation_resume_continuation.scoop` 暴露本地 `Option.Some(continuation)` 存储/读取 provenance 缺口，以及 same-owner wrapper surface resume 过早按内层 handle completion 返回的问题。已补本地 enum/tuple aggregate payload -> pattern extract continuation route，并让 surface resume owner trampoline 使用 wrapper underlying handle route 约束 completion return；相关单测和 run-pass fixture 已通过。
- `continuation_resume_enum.scoop` 暴露多个 resume boundary 共享同一 wrapper continuation schema 时的 projection 合并问题。已让 wrapper outward continuation schema 同时发布 underlying handle-binder publication，并把 same-owner handle-binder wrapper projections 视为同形 contract；该 fixture 已通过。
- `delegated_property_lazy_init_once_basic.scoop` 暴露 refactor MIR value path 缺少 `scoop.sync.*` runtime intrinsic lowering。已补 mutex/condvar/once/destroy 等 sync intrinsics 的 refactor lowering；delegated lazy 与 `std_sync_basic.scoop` 均已通过。
- `delegated_property_lazy_thread_safety_publication_multi_init.scoop` 继续暴露 threadSpawn/join MIR intrinsic、object property `Shared.state` 访问、以及 object property support 检查缺口。已补 refactor thread intrinsics，并让 generic MIR member access/support 识别 object property access；该 fixture 已通过。
- `unsafe_atomic_int_field_lvalue_llvm.scoop` 暴露 struct field 与 class field lookup 同名时 atomic lvalue 误走 class receiver path。已让普通 member/atomic member place 在 receiver 是 struct 时落到 struct GEP；该 build fixture 已通过。
- `delegated_property_map_backed_basic.scoop` 暴露 metadata struct field type fallback 缺少 `TypeKind` / generic class-ref 处理，以及 layout field fallback 产生 runtime warning。已补 TypeKind/nominal class FQN fallback，并避免已知 fallback 先告警；该 fixture 已通过。
- `delegated_property_observable_raise_does_not_poison_mutex.scoop` 暴露 sync/thread intrinsics 在 effect facts 中仍被当作 DynamicFallback effect boundary，以及 Unit handle completion 投影到 typed continuation answer 时缺少默认 Complete payload。已把 sync/thread runtime intrinsics 纳入 plain compiler intrinsic facts，并为 non-elided Complete 在 Unit completion source 上提供类型默认值；该 fixture 已通过。
- `do_block_multiple_trailing_lambda_boundary.scoop` 暴露 HIR lowering 未为隐式 `it` lambda materialize 参数。已在存在单参数期望函数类型时为无显式参数 lambda 注入 synthetic `it` 参数；该 fixture 已通过。
- `cargo run -p scoop -- test` 当前阻塞在 `effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`：nested escaped-continuation replay 在 inner arm-local handle 后没有继续执行 arm tail，属于新的 continuation replay contract 缺口。
- 已按阻塞处理流程新增前置任务 `P7-T02Y`，并同步 `TODO.md`；`P7-T03` 依赖已改为 `P7-T02Y`，本轮不标记 `P7-T03` 完成。
- 验证：`cargo test --all` 已通过；`cargo clippy --all-targets -- -D warnings` 已通过。`cargo run -p scoop -- test` 仍阻塞在新增的 `P7-T02Y` fixture。
- 下一步：提交本轮修复与 TODO 更新后停止。
