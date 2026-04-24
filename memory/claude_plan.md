# 当前执行计划

## 说明

在未读取仓库当前状态前，先记录本轮的高层执行计划，满足“先写计划，再执行命令/代码”的要求。这里记录的是可公开的任务分析与执行步骤，不包含冗长的内部推理展开。

## 本轮目标

完成 `TODO.md` 中第一个未完成任务；如果在执行过程中发现更早存在的问题、阻塞项、规格不匹配或实现边界缺失，则优先修复该问题，或者将其整理为新的前置任务插入 `TODO.md`，更新 `PLAN.md` 后提交并停止。

## 预定步骤

1. 检查最新一次 Git 提交，确认是否显式提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前项目计划、任务依赖和上下文。
4. 如首个未完成任务过大，先拆分为更小子任务，并同步更新 `TODO.md` 与 `PLAN.md`。
5. 实现当前应执行的首个任务或首个子任务。
6. 运行相关检查与测试；若暴露出既有问题，立即转入修复或将其整理为前置任务。
7. 更新 `memory/claude_plan.md` 记录关键进展、计划调整和结论。
8. 更新 `TODO.md` 与 `PLAN.md`，标记完成情况或重排依赖。
9. 提交本轮变更，提交后停止，不继续下一个任务。

## 当前已知约束

- 必须一次只完成一个任务。
- 不能以规避方式绕过缺失特性、错误行为或规格不匹配。
- 若遇到阻塞，必须把阻塞问题前置到 `TODO.md` 中，而不是继续推进后续任务。
- 需要尽量保证 `cargo fmt`、相关测试、以及 `cargo clippy --all-targets -- -D warnings` 通过；若受环境或现有问题限制，需要在计划和结果中明确记录。

## 待补充

已完成对最新提交、`TODO.md`、`PLAN.md` 的初步检查，补充如下。

## 当前任务定位

- 最新提交：`[T4013] Remove inline keyword and keep @Inline as marker`
- 提交说明未额外点名新的“先修复既有问题”条目，因此继续按 `TODO.md` 顺序执行。
- `TODO.md` 中首个未完成任务是 `T4013R`：Review `inline` 已移除，且 `@Inline` 不再参与控制流语义。

## 针对 T4013R 的具体检查项

1. 复查 parser：
   - `inline` 是否仍可作为合法声明修饰符进入 AST。
   - 若保留词法关键字，是否只用于稳定 removed-syntax 诊断，而不是继续形成有效语法。
2. 复查 annotation/typecheck：
   - `@Inline` 是否只被识别为 built-in compile-time marker。
   - `@Inline` 是否只允许函数目标且不允许参数。
   - 表达式、局部声明等非函数位置上的 `@Inline` 是否稳定报错。
3. 复查控制流语义：
   - lambda 中 `return` 是否统一报 `return_not_in_function_body`。
   - 不存在基于 `@Inline` 或旧 `inline` 的 non-local return 特判。
4. 复查 lowering/codegen/runtime：
   - 不存在 `@Inline` 或 legacy `inline` 参与 lowering/codegen/runtime 语义分支的遗留代码。
5. 复查文档与回归：
   - `SCOOP_FULL_SPEC.md`、`sysroot/core.scoop`、`ISSUES.md`、`PLAN.md` / `TODO.md` 叙事一致。
   - 对应 parse/typecheck fixtures 仍覆盖 removed-syntax、target gate 与 non-local return 边界。

## 预定验证

- 定向检查相关源码与 fixtures。
- 运行：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/parse`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 若 review 发现既有问题：
  - 先修复；若无法在本轮直接修复，则把前置任务插入 `TODO.md`、更新 `PLAN.md` 后提交并停止。

## 当前结论与进展

- 已完成代码复扫：
  - parser/lexer/AST：`inline` 仅保留为 removed-syntax 诊断入口。
  - annotation/typecheck：`@Inline` 仅是 built-in compile-time marker，且只允许函数目标、无参数。
  - 控制流：lambda `return` 统一报 `scoop::typecheck::return_not_in_function_body`，没有发现 `@Inline` 或 legacy `inline` 的 non-local return 旁路。
  - lowering/codegen/runtime：未发现 `@Inline` 参与生产语义的 special-case。
- 已完成验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/parse` -> `fixtures: ok (123)`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck` -> `fixtures: ok (394)`
  - `cargo run -p scoop -- test` -> `fixtures: ok (1197)`
  - `cargo test --all` -> 通过
  - `cargo clippy --all-targets -- -D warnings` -> 通过
  - `cargo run -p scoop_tools -- spec-fixtures check` -> `spec fixtures: ok (1)`
- 过程记录：
  - `cargo run -p scoop -- test` 在执行期间长时间无输出，但经进程排查确认 runner 仍在推进不同 fixture，最终正常通过；未发现需要前置修复的新 blocker。
- 下一步收尾：
  - 标记 `T4013R` 完成，更新 `PLAN.md` / `TODO.md`。
  - 检查工作区差异。
  - 提交本轮变更并停止。
