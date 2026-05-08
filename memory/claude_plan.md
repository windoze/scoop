# 执行计划

说明：按安全与协作要求，此文件记录可执行计划、关键决策与进度更新，不记录私有推理细节。

## 初始计划

1. 读取 `TODO.md`，定位第一个标题未标记为 `[DONE]` 的任务，并确认其依赖、约束、验证要求与完成记录。
2. 检查最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题；如有且构成前置条件，则先在 `TODO.md` 中反映该前置关系。
3. 阅读与当前任务直接相关的代码、测试、文档与计划文件，仅收集完成该任务所需的最小上下文。
4. 判断当前任务能否在本次调用中完整落地：
   - 若可以，直接实现任务。
   - 若存在阻塞且必须新增前置任务，最小化更新 `TODO.md`（必要时更新 `PLAN.md`），记录阻塞原因并停止。
5. 对实现结果执行任务要求的验证与回归测试；若失败则立即修复并重新验证。
6. 更新文档与任务记录：
   - 将已完成任务在 `TODO.md` 标题中标记为 `[DONE]`。
   - 补全完成记录、验证命令与必要说明。
   - 仅在阶段计划真实变化时更新 `PLAN.md`。
7. 检查工作区状态，保留与本任务相关的所有变更，按要求创建一次 git 提交，然后停止，不继续处理下一个任务。

## 进度日志

- 已创建初始计划文件；下一步读取 `TODO.md` 确定当前任务。
- 已定位首个未完成任务为 `CG-T07S0a12`，最近提交 `3de3e56e [CG-T07S0a11] ...` 与当前任务无新增未完前置事项。
- 已复现失败：`cargo run -p scoop -- build tests/fixtures/run-pass/operator_overload_struct_basic.scoop -o /tmp/operator_overload_struct_basic` 报 `unsupported value coercion from Int(...) to Struct(...)`。
- 已通过 `dump-mir` 确认根因：HIR 已把 `lhs < rhs` 改写成 `Num.compareTo(lhs, rhs) < 0`，但 MIR lowering 又基于同一 call-site binding 再次触发 compareTo 语法糖，错误生成第二次 `Num.compareTo(compare_result, 0)`。这会把第一次 direct-call 的 `Int` 结果错误重新送入 struct compareTo 路径。
- 当前修复计划：
  1. 在 `crates/scoopc/src/mir/lower.rs` 收紧 `try_lower_compare_to_binary_expr()`，遇到已是 `compareTo(...) < SynthInt(0)` 的 canonical HIR 形状时不再重复改写。
  2. 强化现有 compareTo MIR 单测，断言只生成一次 compareTo direct-call。
  3. 新增或扩展 production LLVM 回归测试，确保 compareTo 结果继续按 `Int` 与 `0` 比较并可成功产出 IR。
  4. 运行任务要求的 build / 单 fixture / full-suite / 格式化 / clippy 验证。
- 已完成代码修复：
  - `mir::lower::try_lower_compare_to_binary_expr()` 现在会识别 canonical HIR 形状 `compareTo(...) < SynthInt(0)`，避免在 MIR 阶段重复套用 compareTo 语法糖。
  - compareTo MIR 单测已强化为“只允许一次 direct-call”；production LLVM 测试已补充 direct-call 次数断言。
- 已完成验证：
  - `cargo test -p scoopc dump_mir_lowers_user_defined_compare_to_as_direct_call_plus_zero_compare`
  - `cargo test -p scoopc dump_mir_lowers_compare_to_in_if_condition_as_direct_call`
  - `cargo test -p scoopc frontend_codegen_consumes_compare_to_direct_calls_without_eager_member_inclusion`
  - `cargo fmt --all`
  - `cargo run -p scoop -- build tests/fixtures/run-pass/operator_overload_struct_basic.scoop -o /tmp/operator_overload_struct_basic`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/operator_overload_struct_basic.scoop`
  - `cargo run -p scoop -- test` 已越过 `operator_overload_struct_basic.scoop`，下一处失败变为 `tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop`
  - `cargo clippy --all-targets -- -D warnings`
- 新 blocker 记录：
  - 定向复现 `safe_member_access_ref_and_extension_basic.scoop` 后，`build` 报 `materialized MIR \`main\` contains rvalue todo \`ctor call lowering pending\` in Some(bb3) at 800..802`。
  - `dump-hir` 显示 safe-call desugaring 生成的 `when` arm body 仍把 `Some(...)` / `None` 保留为 `UnresolvedIdent`，未消费 authoritative `Option` variant ctor/value contract；应在 `TODO.md` 中新增紧随其后的 prerequisite 任务，并保持 `CG-T07S0a` 未完成。

## 本次调用计划（2026-05-08 / CG-T07S0a13）

当前目标：完成 `CG-T07S0a13`，修复 `safe_member_access_ref_and_extension_basic.scoop` 中 safe-call 产物的 `Option.Some` / `Option.None` 构造仍退化为 `ctor call lowering pending` 的问题。

已确认事项：

1. `TODO.md` 中首个未完成任务是 `CG-T07S0a13`。
2. 最近提交信息是 `[CG-T07S0a12] Fix compareTo direct-call lowering`；它与当前任务直接相邻，但未声明新的未完成前置问题。当前 blocker 仍以 `CG-T07S0a13` 本身为准。
3. 当前任务要求修复 authoritative safe-call / ctor lowering 主线，不能在 materialized MIR 或 LLVM backend 现场私补 `Option` 构造。

执行步骤：

1. 定向复现 `safe_member_access_ref_and_extension_basic.scoop` 的 build 失败，并收集最小 HIR/MIR 证据，确认 `Some(...)` / `None` 在哪一层仍是 `UnresolvedIdent`。
2. 阅读与 safe member access、safe-call desugaring、variant ctor/value lowering 直接相关的代码路径，定位 authoritative contract 丢失点。
3. 在 authoritative lowering 主线上做最小修复，让 safe-call 生成的 `Option` arm body 进入现有 variant ctor/value lowering，而不是落入 `Rvalue::Todo("ctor call lowering pending")`。
4. 补充或强化最小回归测试，优先覆盖 safe-call 生成的 `Some` / `None` 路径。
5. 运行任务要求的验证：
   - `cargo run -p scoop -- build tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop -o /tmp/safe_member_access_ref_and_extension_basic`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop`
   - `cargo run -p scoop -- test`
   - 以及必要的定向单测/`cargo clippy --all-targets -- -D warnings`
6. 若 full-suite 暴露新的顺序 blocker，则按 `TODO.md` 约束补录新的 prerequisite；否则将 `CG-T07S0a13` 标记为 `[DONE]`，更新完成记录，并提交本次修改。

本次调用进度：

- 已确认当前任务与约束，下一步开始定向复现并定位 authoritative lowering 丢失点。
- 已复现 `safe_member_access_ref_and_extension_basic.scoop` 的 build 失败：materialized MIR `main` 在 safe-call `Some(...)` 分支上报 `ctor call lowering pending`。
- 已定位根因：safe-call / safe member access desugar 合成的 `Some(...)` 与 `None` 节点都未保留外层 `Option<T>` 结果类型，且 `None` 仍是 bare `UnresolvedIdent`；MIR 因此无法走现有 enum variant ctor/value lowering。
- 已完成代码修复：
  1. `crates/scoopc/src/hir/lower/expr.rs` 现在会给 safe-call 合成的 `Some(...)` 保留外层 `Option<T>` 类型，并把 `None` 改为同样保留结果类型的 `None()` ctor 形状。
  2. `crates/scoopc/src/hir/lower/mod.rs` 的 safe-call HIR 单测已扩充，断言 `Some`/`None` 分支都保留 `Option` 结果类型且 `None` 走 0 参 ctor。
  3. `crates/scoopc/src/mir/lower.rs` 新增定向 MIR 回归，断言 `user?.score` 会 lower 成 `Option.Some` / `Option.None` enum variant，而不是 `ctor call lowering pending`。
  4. 已同步更新受影响的 HIR golden：`tests/fixtures/hir/safe_call_not_null_assert.hir`。
- 已完成验证：
  1. `cargo test -p scoopc typed_hir_lowers_safe_member_type_apply_as_safe_direct_call`
  2. `cargo test -p scoopc dump_mir_lowers_safe_member_access_option_result_without_ctor_todo`
  3. `cargo run -p scoop -- build tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop -o /tmp/safe_member_access_ref_and_extension_basic`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/hir/safe_call_not_null_assert.scoop`
  6. `cargo fmt --all`
  7. `cargo clippy --all-targets -- -D warnings`
  8. `cargo run -p scoop -- test`
- 当前结论：`CG-T07S0a13` 的退出条件已满足，full-suite 已越过 `safe_member_access_ref_and_extension_basic.scoop`。
- 新顺序 blocker：full-suite 下一处失败为 `tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop`；`cargo run -p scoop -- build ...` 报 `materialized MIR 'readValue' contains unresolved generic parameter in frame slot ...: T`。已在 `TODO.md` 中新增 prerequisite `CG-T07S0a14`，并将 `CG-T07S0a13` 标记为 `[DONE]`。
- 剩余收尾：检查 git 变更、创建本次任务提交 `[CG-T07S0a13] ...`，然后停止。
