# 本轮执行计划

## 目标

完成 `TODO.md` 中第一个未完成任务；如果在检查最新提交、代码、测试或执行过程中发现任何既有问题，则优先修复该问题，或将其作为前置任务插入 `TODO.md` 后停止。

## 约束

- 先检查最新一次提交是否提到待修复问题。
- 只完成一个任务，然后停止。
- 不接受规避性实现；若发现规范不匹配、功能缺口、回归或临时绕过，必须优先修复或登记为前置任务。
- 变更后必须更新 `TODO.md`、`PLAN.md`，并提交 Git。

## 执行步骤

1. 查看最新一次提交信息，确认是否明确提到已有问题需要先修复。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，核对当前计划与任务依赖。
4. 结合任务内容检查相关代码与测试范围，判断任务是否可以在本轮完整完成。
5. 如果任务过大：
   - 将任务拆分为更小的可执行子任务。
   - 更新 `PLAN.md`。
   - 更新 `TODO.md`，把新子任务放到正确顺序。
   - 执行拆分后的第一个子任务。
6. 在实现前后持续检查是否存在既有问题、规范不匹配、回归或阻塞项：
   - 若能直接修复，则先修复。
   - 若不能在本轮直接修复，则将其写入 `TODO.md` 作为前置任务，更新 `PLAN.md`，提交后停止。
7. 实现当前目标任务。
8. 运行相关格式化、lint、测试命令，修复出现的问题，直到相关检查通过。
9. 更新 `memory/claude_plan.md` 记录关键进展与计划调整。
10. 更新 `TODO.md`，将当前任务标记为完成或在阻塞时按依赖重排。
11. 更新 `PLAN.md`，记录当前状态、完成情况、后续顺序与任何新增前置项。
12. 检查工作区变更，整理提交内容。
13. 使用清晰的提交信息创建 Git 提交。
14. 停止，不继续处理下一个任务。

## 进度记录

- 已创建本计划文件。
- 已检查最新提交：`063444a6 [T5000e1a] Fix dump-ir external template identity`，提交正文没有额外遗留问题说明。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务为 `T5000e1aR Review：确认 dump-ir 单文件路径的 template identity 已脱离“仅当前源文件”假设`。
- review 过程中通过临时复现用例发现一个现存阻塞问题：
  - 用例形态：`fun wrap<T>(value: T) { print(value) }` 且入口调用 `wrap(1)`。
  - 当前 `dump-ir` 输出中，`wrap::<Int>` 的函数体内仍保留 `callee_fqn: "scoop.core.print"`，没有改写到 `scoop.core.print::<Int>`。
  - 同时 materializer 还错误地产生了 `scoop.core.print::<T>` 这类带模板参数的非具体实例请求。
- 当前调整后的执行计划：
  1. 修复 `dump-ir` materializer 对“generic 实例体内继续 direct-call 外部 generic”的 fixed-point 路径。
  2. 确认不会再从 generic template body 的 typecheck 请求中错误 seed 出非具体实例。
  3. 为该回归补测试。
  4. 重新运行相关测试与 review；若通过，再更新 `TODO.md` / `PLAN.md` 并提交。
- 已完成阻塞修复：
  - `mir/materialize.rs` 已新增请求键到 canonical template 的映射，并把 template family 收紧到“当前 root + lambda family”，避免 declaration/body duplicates 破坏 fixed-point；
  - `seed_requests(...)` 已过滤仍含模板参数的非具体实例请求；
  - 已新增回归测试 `monomorph_rewrites_external_generic_calls_to_concrete_instances`。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc monomorph::lower -- --nocapture`
  - `cargo test -p scoopc mir::tests::dump_mir_keeps_generic_functions_as_templates_before_monomorphization -- --nocapture`
  - `cargo run -q -p scoop -- dump-ir <tmp wrap/print case>`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 当前剩余步骤：
  1. 更新 `TODO.md` / `PLAN.md` 完成记录。
  2. 检查工作区并提交本轮任务。
  3. 停止，等待下一轮。
