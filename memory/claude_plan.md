## 当前执行计划

说明：我不会写入私有的逐字思维链，但会在此文件中持续记录可审阅的执行计划、关键判断依据、已完成步骤和后续调整，便于你检查进度。

### 目标

本轮只完成 `TODO.md` 中第一个未完成任务（或其拆分后的第一个子任务），完成后测试、更新文档并提交，然后停止。

### 预定步骤

1. 查看最新一次 Git 提交的提交信息与改动，确认是否提到了任何既有问题。
2. 若最新提交提到既有问题，则先定位并修复这些问题，再继续主任务流程。
3. 阅读 `TODO.md`，找出第一个未完成任务。
4. 阅读 `PLAN.md`，核对该任务的上下文、依赖与是否需要拆分。
5. 如果该任务过大或存在前置缺口，则先更新 `PLAN.md` 与 `TODO.md`，把任务拆成更小的子任务，并只执行新的第一个子任务。
6. 阅读相关代码、测试、规范或文档，确认实现边界。
7. 实现当前要执行的任务，必要时补充或调整测试。
8. 运行与改动相关的验证；若任务要求涉及全局质量门槛，则补跑 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 或其他必要命令。
9. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况或阻塞原因。
10. 使用清晰的提交信息提交本轮改动，然后停止。

### 进行中

- 已完成：建立本轮执行计划文件。
- 已完成：读取最新提交、`TODO.md`、`PLAN.md`，确认当前第一个未完成任务为 `T4002R`。
- 已完成：检查最新提交完整说明；提交信息未额外提到需要先修的既有问题。
- 已完成：审查 `T4002` 的新增回归与关键实现路径。
- 已发现既有问题：`tests/fixtures/run-pass/receiver_lambda_this_shadows_outer_this.scoop` 的注释要求“receiver lambda 的 `this` 遮蔽外层 `this`”，但当前实际运行输出为 `99`，说明 lowering/codegen 仍沿用了 resolver 写回的外层成员解析结果。
- 问题判断：
  - typecheck 已能基于 expected receiver type 对当前 lambda 中的 `this.member` / `this.member(...)` 做晚解析；
  - 但 HIR lowering 多处仍直接读取 AST 上的 `member.resolved`，没有统一读取 typecheck 最终确认的成员决议；
  - 因而在“外层已有 `this` 上下文”的嵌套 receiver lambda 中，前端与 lowering 的语义源发生分裂。
- 下一步：
  1. 为 typecheck 新增“成员最终决议”side table，并在 member access / member call 的晚解析路径写入。
  2. 让 HIR lowering 的相关入口优先读取该 side table，而不是盲信 AST 上 resolver 的旧决议。
  3. 修正并补跑 receiver lambda 遮蔽回归，以及相关定向/全量验证。

### 已完成的关键实现

- 已为 `ast::File` / `TypeLowering` / `check_file_exprs` 补充 `typechecked_member_resolved` side table，用来承接 typecheck 最终确认的成员解析结果。
- 已在普通 member access、safe member access、member call 与 member-assignment lhs 的 typecheck 路径写入这张 side table。
- 已让 HIR lowering 的普通 member lowering、扩展函数调用 lowering、直连成员函数 lowering、effect-op call lowering 与 delegated property assign 优先读取 typecheck 最终决议。
- 已为 HIR lowering 增加 receiver lambda 的当前隐式 `this` 上下文：进入 receiver lambda body 时会覆盖为该 lambda 的合成 `this` 绑定，嵌套普通 lambda 继承、嵌套 receiver lambda 再覆盖。

### 当前状态

- 已完成验证：
  - `target/debug/scoop run tests/fixtures/run-pass/receiver_lambda_this_shadows_outer_this.scoop` 输出已从错误的 `99` 修正为 `3`。
  - `target/debug/scoop test --fixtures target/t4002r-fixtures/infer` → `fixtures: ok (1)`。
  - `target/debug/scoop test --fixtures target/t4002r-fixtures/run-pass` → `fixtures: ok (4)`。
  - `target/debug/scoop test --fixtures tests/fixtures/typecheck` → `fixtures: ok (326)`。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 已完成文档同步：`TODO.md` 与 `PLAN.md` 已把 `T4002R` 标记完成，并把下一项推进到 `T4003`。
- 下一步：检查工作区、提交本轮改动，然后停止。
