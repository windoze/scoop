# 本次执行计划

说明：我不会记录或暴露逐字的内部详细思维过程，但会持续维护可检查的高层计划、关键判断、执行进度与阻塞信息。

## 初始计划

1. 读取 `TODO.md`，确定第一个标题未标记 `[DONE]` 的任务。
2. 查看最近一次提交信息，判断是否存在与该任务直接相关且明确未完成的问题；若有，将其视为当前任务的一部分或按要求写入 `TODO.md` 作为前置。
3. 阅读当前任务在 `TODO.md` 中的完整要求、依赖、验证标准与完成记录，并在必要时查看 `PLAN.md` 了解阶段上下文。
4. 检查工作区现状，避免覆盖我未创建的改动；仅在当前任务相关范围内修改代码。
5. 实现当前任务，优先做最小且正确的改动；若遇到阻塞当前任务的真实缺口或规格不匹配，先修复，或将最小前置任务写入 `TODO.md` 并停止。
6. 运行与当前任务相关的验证，包括必要测试，并尽量满足仓库要求的格式、lint、编译与任务指定验证。
7. 更新 `memory/claude_plan.md` 记录关键进展、计划调整与验证结果。
8. 按要求更新 `TODO.md`：若任务完成，则给任务标题加上 `[DONE]` 并补全完成记录；若被阻塞，则在 `TODO.md` 中插入最小前置任务并保持当前任务未完成。
9. 仅在阶段计划、依赖关系或完成标准发生变化时更新 `PLAN.md`。
10. 将本次任务涉及的全部未提交改动一并提交，提交信息使用当前任务 ID。
11. 完成一个任务后立即停止，不继续处理下一个任务。

## 进度记录

- 已写入初始计划，下一步读取 `TODO.md` 并确定当前任务。
- 已确认当前首个未完成任务为 `CG-T07S0a9`：修复 `member_call_devirt_final_receiver_direct_call_basic.scoop` 中 final receiver direct-call 去虚化后，`Base` vtable 仍引用未发射的 `Base.ping` 符号的问题。
- 已查看最近一次提交：`[CG-T07S0a8] Preserve tuple args in effect-step carrier forwarding`。提交信息未显式声明与 `CG-T07S0a9` 直接相关的额外未完成前置问题。
- 下一步：
  1. 复现 `member_call_devirt_final_receiver_direct_call_basic.scoop` 的 build/test 失败。
  2. 检查去虚化、method publication、vtable emission 与 callable reachability 相关实现，确认 authoritative contract 在哪里丢失了 `Base.ping`。
  3. 做最小修复并补最小回归测试。
  4. 运行任务要求的验证，随后更新 `TODO.md` 与提交变更。

- 已复现失败：fixture 链接阶段报 `_Base.ping` undefined symbol，`__scoop_vtable__Base` 仍引用该符号。
- 已定位根因：LLVM 可达性收集在 materialized MIR 路径扫描 `Rvalue::ClassCtor` 时只扫描了参数，没有像 HIR ctor 路径那样把 ctor/class reachability 接回主线，因此 `Base` vtable 需要的 `Base.ping` 未进入 body 发射集合；但 `Derived` type descriptor 仍会级联发布 `Base` vtable，从而留下 declaration-only `Base.ping`。
- 已实施修复：
  1. 在 `crates/scoopc/src/llvm/reachability.rs` 中让 `Rvalue::ClassCtor` 显式 `enqueue_ctor(class_fqn, selected_ctor_span)`，把 ctor/super/vtable/itable reachability 接回 MIR 主线。
  2. 在 `crates/scoopc/src/llvm/tests.rs` 的既有去虚化测试中增加断言：`Base` vtable 继续发布，且 `Base.ping` 必须被定义而不是只声明。
- 下一步：运行定向测试、fixture build/test 与默认 full-suite；若都通过，再更新 `TODO.md` 并提交。
- 验证结果：
  1. `cargo test -p scoopc via_mir_direct_class_call_is_not_reinterpreted_as_vtable_dispatch` 通过。
  2. `cargo run -p scoop -- build tests/fixtures/run-pass/member_call_devirt_final_receiver_direct_call_basic.scoop -o /tmp/member_call_devirt_final_receiver_direct_call_basic` 通过。
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_devirt_final_receiver_direct_call_basic.scoop` 通过。
  4. `cargo clippy --all-targets -- -D warnings` 通过。
- 默认 full-suite 结果：`cargo run -p scoop -- test` 已越过 `member_call_devirt_final_receiver_direct_call_basic.scoop`，下一处失败变为 `tests/fixtures/run-pass/nothing_raise_coerce_to_any_type.scoop`。
- 新 blocker 诊断：单 fixture build 报 `refactor callable 'main' step schema s0 (ABI s0) state st9 terminator lowering failed: ... boundary bd1 case c0 命中多个 HandleDispatch routing contract`；该问题属于后续任务前置，已写入 `TODO.md` 作为新的 prerequisite `CG-T07S0a10`。
- 当前状态：`CG-T07S0a9` 已完成；下一步整理提交并停止。
