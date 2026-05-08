## 当前执行计划

注意：这里记录的是对外可见的执行计划与进度摘要，不包含内部私有推理细节。

1. 在不做开放式排查的前提下，先读取 `TODO.md`，定位第一个标题未标记 `[DONE]` 的任务。
2. 读取与该任务直接相关的上下文，包括 `PLAN.md`、最近提交信息，以及任务涉及的代码与测试位置。
3. 判断该任务是否可直接完整实现；若存在阻塞且必须先补充前置任务，则最小化更新 `TODO.md`/必要时更新 `PLAN.md`，提交后停止。
4. 若无阻塞，按任务要求做最小且正确的实现，不引入规避性方案。
5. 运行该任务要求的验证，以及相关测试、格式化、lint/构建检查；若失败则继续修复直到通过，或确认存在必须前置的新阻塞任务。
6. 完成后更新 `TODO.md`：将该任务标题前缀改为 `[DONE]`，补充完成记录；仅当阶段计划确实变化时才更新 `PLAN.md`。
7. 将本次相关改动提交到 Git，提交信息使用当前任务号，随后停止，不进入下一个任务。

## 当前任务

- 已从 `TODO.md` 确认首个未完成任务为 `CG-T07S0a20`：修复 `tests/fixtures/run-pass/string_trim_indent_basic.scoop` 中运行期 `String.trimIndent()` builtin member 调用仍退化成 unresolved `MemberAccess` + `FunValue` callee 的问题。

## 细化执行步骤

1. 读取 `CG-T07S0a19` 相邻修复涉及的 String builtin member 调用实现，找出 `trimIndent` 仍未接入 authoritative contract 的缺口。
2. 检查最近提交与当前工作树，确认没有需要并入当前任务的直接未完事项。
3. 在 resolve/typecheck/HIR/MIR/materialize/codegen 链路中做最小修复，让运行期 `String.trimIndent()` 走 typed direct/member/intrinsic call，而非 unresolved member + `FunValue`。
4. 增补最小回归测试，至少覆盖运行期 `String.trimIndent()` member call lowering contract。
5. 运行任务要求的构建/fixture/default full-suite 验证；若暴露与当前任务直接相关的新 blocker，则按要求更新 `TODO.md` 并停止，否则继续直到验证通过。
6. 更新 `TODO.md`：将 `CG-T07S0a20` 标记为 `[DONE]` 并补充完成记录；如阶段计划未变则不改 `PLAN.md`。
7. 提交本次改动并停止。

## 进度

- 已写入初始执行计划。
- 已读取 `TODO.md` 并锁定当前任务 `CG-T07S0a20`。
- 已检查当前工作树与最近提交：`HEAD` 为 `Update plan`，未发现需要先并入或补录的直接未完事项；工作树中的未提交改动集中在 `trimIndent()` contract 修复相关文件，属于当前任务续做现场。
- 已确认实现面覆盖当前任务主链路：typecheck 为 `String.trimIndent()` 发布 extension-style direct-call contract，legacy LLVM dispatch 与 refactor direct-call lowering 接入 `scoop.core.trimIndent`，并新增定向编译器回归测试。
- 已通过定向验证：`cargo test -p scoopc builtin_string_trim_indent_member_calls_lower_to_direct_calls -- --nocapture`、`cargo run -p scoop -- build tests/fixtures/run-pass/string_trim_indent_basic.scoop -o /tmp/string_trim_indent_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/string_trim_indent_basic.scoop`。
- 已通过质量门禁：`cargo clippy --all-targets -- -D warnings`。
- 已运行默认 `cargo run -p scoop -- test`：full-suite 已越过 `string_trim_indent_basic.scoop`，当前停在后续已存在 blocker `tests/fixtures/run-pass/task_step_concurrent_running_trap.scoop`（对应 `CG-T07S0a23`），因此无需新增前置任务。
- 下一步：审阅 `TODO.md`/`git diff`，确认账本更新无误后提交本次任务改动并停止。
