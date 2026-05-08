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
