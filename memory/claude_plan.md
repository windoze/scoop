## 执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断完成状态，定位第一个未完成任务。
2. 检查最近提交是否有与该任务直接相关且明确未完成的问题；若有，将其视为当前任务的一部分或作为 `TODO.md` 中的前置依赖处理。
3. 阅读当前任务涉及的代码、测试、规范与依赖文件，确认实现边界与验证要求。
4. 在不引入规避方案的前提下完整实现当前任务；若遇到阻塞当前任务的真实缺口或回归，先修复该问题，或把最小必要前置任务写入 `TODO.md` 并停止。
5. 运行与当前任务直接相关的验证，包括要求的测试、必要的构建检查，以及 `cargo clippy --all-targets -- -D warnings`（若当前改动会影响其结果）。
6. 根据执行结果更新 `memory/claude_plan.md`、`TODO.md`，必要时才更新 `PLAN.md`。
7. 若任务完成，则把该任务标题标记为 `[DONE]`，补全完成记录，并创建一次 git 提交；若因阻塞只能调整任务列表，也提交这些变更后停止。

## 进度记录

- 已读取 `TODO.md` 并确认首个未完成任务是 `CG-T07S0a10`：修复 `nothing_raise_coerce_to_any_type.scoop` 中 nested try/catch + `Raise.raise(...)` 导致的 `HandleDispatch` routing contract 歧义。
- 已检查最近提交：`[CG-T07S0a9] Restore MIR ctor reachability for vtable methods`。该提交的完成记录已在 `TODO.md` 中把下一处默认 full-suite blocker 记录为 `CG-T07S0a10`，因此继续按当前任务顺序执行。
- 当前工作计划：
  1. 复现 `tests/fixtures/run-pass/nothing_raise_coerce_to_any_type.scoop` 的单 fixture build/test 失败，并抓取 diagnostics。
  2. 搜索 `HandleDispatch routing contract`、boundary/case routing 发布与验证代码，锁定重复发布的 producer。
  3. 做最小修复，确保同一 boundary case 只保留单一 authoritative routing contract，且不改变 `Nothing` / nested catch 语义。
  4. 运行任务要求的定向验证；若默认 full-suite 前进到新的 blocker，再按顺序在 `TODO.md` 中记录并停止在本任务完成处。
  5. 更新 `TODO.md` 的 `CG-T07S0a10` 为 `[DONE]` 并补完成记录，必要时同步 `memory/claude_plan.md`。
  6. 提交本次改动并停止。

- 已完成复现：`cargo run -p scoop -- build tests/fixtures/run-pass/nothing_raise_coerce_to_any_type.scoop -o /tmp/nothing_raise_coerce_to_any_type` 复现 `refactor boundary bd1 case c0 命中多个 HandleDispatch routing contract`。
- 已定位根因：LLVM `HandleDispatch` 选择逻辑把 `LateLoweredHandleStateRegion::Exit` 错算为动态嵌套区域，导致一个顺序上更早的 sibling `handle` exit state 人为抬高了外层 nested try/catch handler 的深度，和真正的内层 handler 打平，从而在 `bd1/c0` 上报多重 contract。
- 已完成修复：在 `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 中新增 `handle_dispatch_region_implies_runtime_nesting()`，把运行期嵌套判定收紧为真实包围区域（排除 `Exit`）；同步让 `surface_resume_allows_handle_dispatch()` 与 `handle_dispatch_nesting_depth()` 共用该规则。
- 已补回归：新增 `llvm::tests::nested_raise_try_catch_uses_innermost_handle_dispatch_contract`，覆盖 nested `Raise.raise` + nested try/catch 的最内层 handler 选择。
- 已完成验证：
  1. `cargo test -p scoopc nested_raise_try_catch_uses_innermost_handle_dispatch_contract`
  2. `cargo run -p scoop -- build tests/fixtures/run-pass/nothing_raise_coerce_to_any_type.scoop -o /tmp/nothing_raise_coerce_to_any_type`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/nothing_raise_coerce_to_any_type.scoop`
  4. `cargo run -p scoop -- test`
  5. `cargo fmt`
  6. `cargo clippy --all-targets -- -D warnings`
- 默认 full-suite 已越过 `nothing_raise_coerce_to_any_type.scoop`，下一处失败转为 `tests/fixtures/run-pass/object_companion_value_named_nested_init_basic.scoop`；已按顺序把该问题登记为 `CG-T07S0a11`，当前 invocation 到此停止，不继续实现下一任务。
