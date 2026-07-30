<<<<<<< Updated upstream
# Claude Execution Plan

## Scope
=======
## 本轮执行计划（P4-T01l）

按照 `PROMPT.md` 规范完成 `TODO.md` 中第一个未完成任务 **P4-T01l**：解锁 `@Intrinsic struct/class` body method 在 builtin scalar receiver / `ToString.toString` interface dispatch 上的可达性。
>>>>>>> Stashed changes

This invocation will complete exactly the first incomplete task listed in `TODO.md`, then stop after documenting and committing the result. `TODO.md` is the source of truth for task ordering and completion status.

<<<<<<< Updated upstream
## Reasoning Summary

- Read `TODO.md` first to identify the first task whose heading is not prefixed with `[DONE]`.
- Check the latest commit only for unfinished work directly relevant to that first incomplete task.
- Avoid broad triage or unrelated historical cleanup.
- If the task can be completed as written, implement it fully, validate it, mark it `[DONE]`, update its completion record, and commit.
- If a concrete blocker prevents spec-correct completion, add the minimum prerequisite task in `TODO.md`, leave the current task incomplete, commit the scheduling change, and stop.

## Step-By-Step Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Inspect only the task-relevant context, including the latest commit message if it points to unfinished work for that task.
3. Read the relevant source, tests, fixtures, and specification sections needed for the selected task.
4. Implement the smallest correct change that satisfies the selected task without workarounds or spec deviations.
5. Add or update tests/fixtures required by the task.
6. Run validation in the required order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, relevant tests, then full tests and fixtures when required.
7. If validation exposes unscheduled failures, fix them if in scope or add the minimum prerequisite/follow-up task before marking completion.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling in the completion record.
9. Update this file whenever the plan changes or a key step completes.
10. Inspect git status/diff/log, stage only intended files, commit with a task-tagged message, and stop.

## Progress Log

- Initial plan written before reading `TODO.md` or running commands.
- Identified the first incomplete task as `TC-04-FIX3: 清除 source-callable/direct-call 残留 FQN live lookup`.
- Checked the latest commit: `[TC-04-R] Schedule source callable FQN lookup fix`; it is directly relevant and already represented by `TC-04-FIX3`, so no extra prerequisite is needed before implementation.
- Added handle-native ABI/signature entry points for `LirCallableRef` / exact callee bindings and began migrating direct-call, dispatch, closure-body, release-hook, and layout query call sites away from the old FQN-named helpers.
=======
- 上次提交 `[P4-T01]` 已显式拆出 `P4-T01l` 作为前置；`P4-T01l` 是本轮要完成的任务。
- `P4-T01` 仍是 `[TODO]`，等 `P4-T01l` 完成后再启动。

### 实现策略

本任务严格按 "新机制双轨可达，不删除既有 by-name 拦截" 的硬约束推进。这次会改为更小步骤的探索与验证：

1. **先重做 sysroot 改写（与上次会话相同）**：把 `Bool/Char/Int/Float64/Float32/String` 改成 `@Intrinsic struct/class with body methods`。
2. **不动 typecheck 的 by-name short-circuit**：旧路径继续负责 `Int.toString()` / `Bool.toString()` etc. 的 typecheck 与 codegen——这是上次会话最大的认知错误，本次保留。
3. **只在 typecheck "下游路径" 上把 `<TypeFqn>.<methodName>` body method 也作为可达性"备选"**：当 by-name short-circuit 之外的路径（例如 `ToString.toString` interface dispatch）触达时，让 monomorphization / itable / late-lowering 能正确发现 sysroot bodied helper。
4. 关键：让 `ToString.toString` 走 itable 时，对 builtin scalar override，把 `<scalar>.toString` body method 作为 itable slot target 发布出来（与 user-typed override 同构）。
5. 验证：scalar `42.toString()` / `true.toString()` 仍然走旧 by-name 路径（不要回退）；`println(42)` 走 ToString.toString itable dispatch 时能找到发布出来的 callable body（即 `Int.toString` 等的 sysroot body）。
6. 跑全量回归：`cargo test -p scoopc` / `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` / `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck` / `cargo clippy --all-targets -- -D warnings`。

### 顺序

1. 重新改写 sysroot（与上次会话相同的 6 个 `@Intrinsic struct/class with override fun toString` 形态）。
2. 跑 `cargo build -p scoopc` 与 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 看 baseline 还能不能通过——原始 by-name 拦截全部保留，所以应该通过。
3. 加 owner test（一个最小的 `println(42)` style fixture）锁定"interface dispatch 必须发布 callable body"。
4. 找到 `ToString.toString` callable publish 的 monomorphization / late-lowering 入口，看为什么 builtin scalar override 没有被发布。
5. 修复发布路径，让 builtin scalar override 进入 itable & late-lowered body 通道。
6. 加 fixture 验证 `println(<scalar>)` 不退化、不再触发 "published late-lowered body" 缺失。
7. 写完成记录、commit。

### 风险点

- 若 itable 发布机制本身就强依赖 `Nominal` 类型 receiver，对 builtin scalar 完全不通，那 P4-T01l 可能要扩展 itable 收集 / late-lowering 入口的入参范围；这可能涉及多处。
- 若 `ToString.toString` 默认 body 在没有 builtin scalar override 时 work，但有 override 时反而失败——那意味着 override 的发布逻辑漏了 builtin scalar；针对性修这一段就够了。

### 进展更新

（执行过程中持续追加。）
>>>>>>> Stashed changes
