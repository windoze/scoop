## 当前计划

1. 先读取 `TODO.md`，严格按标题是否带 `[DONE]` 判断首个未完成任务。
2. 读取最近一次提交信息，确认是否有与该任务直接相关且明确未完成的问题；若有，则将其视为当前任务的一部分或在 `TODO.md` 中补成前置任务。
3. 阅读当前任务涉及的说明、依赖、验证要求，以及相关代码与测试，避免做开放式排查。
4. 以最小且正确的改动完成该任务；如果遇到阻塞当前任务的真实缺口或缺陷，不做变通，而是在 `TODO.md` 中补最小前置任务并停止。
5. 运行任务要求的验证，以及必要的 `cargo fmt`、相关测试、`cargo clippy --all-targets -- -D warnings`；若失败则先修复。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
7. 在 `TODO.md` 中把完成的任务标题前加上 `[DONE]`，并补全 completion record；仅在阶段计划真实变化时更新 `PLAN.md`。
8. 按仓库提交风格创建一次 git commit，提交当前任务相关的全部未提交改动，然后停止，不进入下一个任务。

## 进度记录

- 已写入初始执行计划，下一步开始读取 `TODO.md` 并识别首个未完成任务。
- 已确认首个未完成任务为 `CG-T07S0a14`：修复 `smart_cast_any_member_access_generic_class_basic.scoop` 中 smart-cast 分支 generic class field access 仍把 `x.value` 的 result/frame slot 保留为 unresolved `T`。
- 最近一次提交是 `[CG-T07S0a13] Restore safe-call Option variant lowering`，未显式记录与 `CG-T07S0a14` 直接相关的未完成问题，因此继续按 `CG-T07S0a14` 本身执行。
- 当前工作树仅有本文件未提交改动，暂未发现其他脏改动需要纳入当前任务处理。
- 下一步：
  1. 复现 `tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop` 的 build/test 失败。
  2. 用定向 `dump-mir` / 代码搜索定位 smart-cast 分支里 `MemberAccess` 与 result/frame slot 仍保留 declaration-site `T` 的发布路径。
  3. 以最小改动修正 authoritative smart-cast/member-access contract，使 materialized MIR 中 `x is Box<Int>` 分支的 `x.value` 具体化为 `Int`。
  4. 运行任务要求的验证、更新 `TODO.md` 完成记录、提交 git commit，然后停止。
- 已完成 `CG-T07S0a14` 实现：
  1. `crates/scoopc/src/mir/lower.rs` 的 member-access lowering 现在会优先保留 typed HIR 已具体化的结果类型，只在合成 HIR 仍给出宽的 `Any` 时回退到成员声明类型。
  2. member-access receiver 只会在 `receiver.ty` 比底层 local 更具体且不是 `Any` 时创建 expr-typed 视图 local，修复 smart-cast 分支 `Box<Int>.value` 具体化，同时避免破坏 extension-property getter / `with` builder。
  3. 新增 `mir::lower::tests::dump_mir_smart_cast_member_access_preserves_concrete_generic_field_type`；同步更新 `tests/fixtures/mir_refactor/generic_materialization.mir`。
- 已完成验证：
  1. `cargo test -p scoopc dump_mir_smart_cast_member_access_preserves_concrete_generic_field_type`
  2. `cargo run -p scoop -- build tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop -o /tmp/smart_cast_any_member_access_generic_class_basic`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/extension_property_getter_basic.scoop`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/comptime_splice_class_with_update.scoop`
  6. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/generic_materialization.scoop`
  7. `cargo clippy --all-targets -- -D warnings`
- 默认 `cargo run -p scoop -- test` 已越过 `smart_cast_any_member_access_generic_class_basic.scoop`，新的下一处 blocker 是 `tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop`：`scoop.collections.__map_alloc_empty_table` 的 array transport element type 仍保留 unresolved `T`。
- 已按顺序在 `TODO.md` 中新增前置任务 `CG-T07S0a15`，本次提交将以完成 `CG-T07S0a14` 并补录下一 blocker 为止，不继续实现 `CG-T07S0a15`。
