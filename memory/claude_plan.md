# 本轮执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 执行边界

- 先检查最新一次提交是否提到了需要优先修复的既有问题；如有，先处理这些问题。
- 再读取 `TODO.md`，确定第一个未完成任务。
- 如果该任务过大或被前置缺陷阻塞，则先在 `PLAN.md` / `TODO.md` 中拆解或重排依赖，并以新的第一个子任务作为本轮目标。
- 不接受规避实现、临时垫片或仅为夹具服务的 hack；如果发现与规范不符的缺口，必须先把缺口转成更靠前的任务。
- 本轮结束前需要完成实现、验证、文档更新和一次 git 提交；然后停止，不进入下一项任务。

## 计划步骤

1. 检查最新提交信息，确认是否存在提交信息中提到但尚未修复的问题。
2. 读取 `TODO.md` 与 `PLAN.md`，识别当前优先级最高的未完成事项。
3. 结合代码现状评估该事项是否可以在本轮完整落地；若不能，则拆解任务并更新计划文件。
4. 实施代码修改，确保实现符合规范而不是依赖变通方案。
5. 运行必要的格式化、测试和 `cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录结果、依赖和剩余风险。
7. 提交本轮变更，提交信息引用对应任务编号，然后停止。

## 记录约定

- 每完成一个关键步骤，追加或修改本文件中的进展记录。
- 如果发现阻塞项，会在这里记录阻塞原因、受影响任务和 `TODO.md` 中的新排序。

## 当前进展

- 已检查最新提交 `56d73c47e75fe7904b87af8848e527831fafccad`（`[T3103a0] Restore statement-position call gates`）。提交信息未声明需要在本轮开始前额外修复的遗留 issue，因此无需插入新的“先修已知问题”步骤。
- 已读取 `TODO.md` / `PLAN.md`，确认当前首个未完成任务为 `T3103a`：收口 `@Safe` / `@Unsafe` 与 `do` / closure 的绑定规则。
- 已确认本任务在当前上下文下可以直接执行，不需要先拆分成新的子任务；其依赖 `T3103a0` 已完成。
- `T3103a` 的实现已完成：
  - parser 现已只接受 `@Unsafe do { ... }` 作为局部 unsafe block，裸 `@Unsafe { ... }` 会报稳定的 `scoop::parse::unsafe_block_requires_do`。
  - `@Safe { ... }` 已改为 annotated closure；AST `LambdaExpr` 与 HIR `ClosureExpr` 新增 `at_safe_span`。
  - typecheck 已通过 `with_safe_lambda_context` 将 safe closure 的 unsafe-context 抑制接回 lambda 推导、statement-position lambda 检查和 delegated-property callback 检查。
  - `sysroot/string.scoop` 与相关 fixtures 中仍把 bare annotated block 当局部 block 使用的代码已迁移到 `do` 形式。
- 新增回归已落地：
  - parse：`safe_do_block_vs_safe_closure.*`、`unsafe_block_requires_do_fail.scoop`
  - HIR：`safe_closure_basic.*`
  - unsafe/nogc：`safe_closure_inside_unsafe_fun_requires_unsafe_is_error.scoop`、`safe_closure_nested_unsafe_do_allows_extern_ok.scoop`
- 已完成验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/unsafe_nogc`（`fixtures: ok (33)`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/parse`（`fixtures: ok (117)`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/hir`（`fixtures: ok (15)`）
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (1005)`）
  - `cargo clippy --all-targets -- -D warnings`
- 下一项未完成任务已推进为 `T3104`。

## 本轮实现计划（细化）

1. 调整 parser 与 AST：
   - `@Unsafe do { ... }` 继续解析为局部 unsafe block。
   - `@Unsafe { ... }` 改为稳定语法错误，并明确要求使用 `@Unsafe do { ... }`。
   - `@Safe do { ... }` 继续解析为局部 safe block。
   - `@Safe { ... }` 改为 annotated closure，而不是 `SafeBlock`。
2. 调整 HIR / typecheck：
   - 在 AST/HIR 中显式保留 safe closure 标记，避免再次把它吞成普通 block。
   - 在 lambda body typecheck / expected-type 推导处正确暂停外层 unsafe context，使 `@Safe { ... }` 的 body 按 safe 语义检查。
3. 迁移仓库中的旧写法：
   - 把 sysroot 与 fixtures 中仍把 `@Safe { ... }` / `@Unsafe { ... }` 当局部 block 使用的代码迁移到 `@Safe do { ... }` / `@Unsafe do { ... }`。
   - 同步更新 parser 单测、源码注释与新增回归。
4. 验证并收尾：
   - 运行相关 parser/typecheck/fixture 测试与 `cargo clippy --all-targets -- -D warnings`。
   - 更新 `TODO.md` / `PLAN.md` / 本文件并提交本轮变更。
