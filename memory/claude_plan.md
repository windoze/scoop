# 执行计划与进度记录

说明：这里记录可审阅的执行计划、关键判断、执行进度与变更原因。我不会写入逐字逐句的内部推理，但会完整记录可复核的步骤、结论和后续动作。

## 当前目标

按 `TODO.md` 的顺序完成第一个未完成任务；在开始该任务前，先检查最新提交是否提到任何既有问题，如有则先修复这些问题。

## 初始执行计划

1. 查看最新一次 Git 提交信息，确认是否提到待修复的既有问题。
2. 打开并阅读 `TODO.md`，定位第一个未完成任务。
3. 打开并阅读 `PLAN.md`，核对该任务背景、依赖和已有计划。
4. 结合代码库现状评估任务复杂度：
   - 若任务可在本轮完整完成，则直接实现。
   - 若任务过大，则将其拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
5. 实现任务，必要时补充或调整测试。
6. 运行与该任务相关的验证：
   - 至少运行定向测试；
   - 若改动影响面较大，再运行更广的测试/检查；
   - 按要求关注 `cargo clippy --all-targets -- -D warnings` 是否存在告警。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中将本轮完成的任务标记为已完成；
   - 在 `PLAN.md` 中记录当前状态与后续影响；
   - 持续更新本文件中的关键进展。
8. 使用清晰的 Git 提交信息提交本轮变更，然后停止。

## 进度日志

- 已创建本文件，准备开始检查最新提交与任务列表。
- 已检查 `git log -1 --stat --decorate=short --oneline`：
  - 最新提交为 `e9a97a0 [T4001R] 复审参数化超类型与 star projection 主线`。
  - 从提交标题与变更摘要看，未直接提到新的既有缺陷或待修问题。
- 已读取 `TODO.md` 与 `PLAN.md`：
  - 当前第一个未完成任务是 `T4002`：补齐 lambda expected-type 推断与 receiver lambda 基本语义。
  - 当前尚未判断是否需要拆分；下一步先读取 `ISSUES.md` 与相关代码实现位置。
- 已读取 `ISSUES.md` 第 3 条与 `infer.rs` / `resolve/scopes.rs` / `member.rs` / `hir/lower/expr.rs` 的相关实现，形成当前判断：
  - `T4002` 在本轮可直接实现，不需要先拆子任务。
  - 现有问题不止“0/1/2 参数限制”和“receiver lambda 缺少 this”；还包含一个同属该条目的真实门禁：即使 lambda 参数都显式标注了类型，只要没有 expected type，当前也会被直接拒绝。
  - HIR lowering 与 LLVM codegen 并非完全阻塞：只要把 lambda 的最终函数类型写回并在 lowering 中读取，就能支撑“显式参数类型 + 无 expected type”的最小可执行场景。

## 当前实施方案

1. 修改 lambda expected-type 推断：
   - 把 `try_infer_lambda_expr_type_by_expected` 从写死 0/1/2 参数改为支持任意参数个数；
   - 保留隐式 `it` 仅适用于“无参数列表 + expected 恰有 1 个普通参数”的规则。
2. 为 receiver lambda 增加 `this` 语义：
   - 在 resolver 中为“可能成为 receiver lambda 的 `this`”预留延后绑定能力；
   - 在 typecheck 中为 lambda body 注入 receiver `this` 的局部类型；
   - 对 `this.member` / `this.method()` 这类场景补一条按 receiver 实际类型 late resolve 的路径，避免 resolver 过早失败或误绑定到外层 `this`。
3. 放宽无 expected type 的总门禁：
   - 对“参数全部显式标注类型”的 lambda，直接根据参数类型 + body 类型推导函数类型；
   - 让 HIR lowering 读取 typecheck 写回的表达式类型，避免局部 `val f = { ... }` 继续退回 `Any`。
4. 新增/更新回归：
   - infer/typecheck：高参数 lambda expected-type 下推；
   - typecheck/run-pass：receiver lambda 中直接使用 `this`；
   - infer/run-pass：显式参数类型的 lambda 在无 expected type 时可定型并可执行；
   - 视需要同步 stdlib / 现有 scope-function fixture，使其体现 receiver lambda 语义。
## 2026-04-19 接续执行记录

### 当前目标
- 继续完成 `TODO.md` 中首个未完成任务 `T4002`，不推进后续任务。
- 先做验证，再根据失败结果修复实现，最后更新 `TODO.md`、`PLAN.md`、本文件并提交。

### 接续后的执行计划
1. 检查当前工作区状态，确认已有改动范围与待验证文件。
2. 运行与 `T4002` 直接相关的定向 fixture 测试，优先覆盖 `run-pass` 与 lambda / receiver function / scope function 相关路径。
3. 若测试失败，修复 receiver lambda `this`、lambda 类型推导或 late member resolve 等问题，并回归相关 fixture。
4. 在定向验证通过后，运行仓库要求的更完整验证，包括 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
5. 全部通过后，更新 `TODO.md`、`PLAN.md` 与本文件，标记 `T4002` 完成并记录验证命令。
6. 使用 `[T4002] ...` 风格提交本轮改动，然后停止。

### 当前已知风险
- `typecheck/expr/call.rs` 中 receiver lambda `this.method(...)` 的 late resolve 与 extension fallback 边界需要重点验证。
- receiver lambda 与外层 `this` 的遮蔽关系需要通过 run-pass fixture 实测确认。
- lowering 读取 typecheck 后的 lambda 函数类型后，需要确认运行期闭包参数绑定没有退化。

### 已发现的实际阻塞
- `target/debug/scoop test --fixtures target/t4002-fixtures/run-pass` 首次失败于
  `kotlin_scope_functions_basic.scoop`。
- 直接复现 `target/debug/scoop build tests/fixtures/run-pass/kotlin_scope_functions_basic.scoop -o /tmp/t4002-kotlin-scope`
  后，错误为 `capture local not found`。
- 已定位为 LLVM closure codegen 仍把 receiver lambda 的隐式 `this` 当成 capture env 项；
  这会在生成闭包对象时去外层环境查找本应来自 LLVM receiver 参数的局部绑定。

### 当前修复动作
- 正在修改 `crates/scoopc/src/llvm/codegen/mod.rs`：
  - 从 closure binding 阶段把 receiver lambda 的 `this` 从 captures 中剥离；
  - 在 closure fun body 入口把 `this` 绑定到 LLVM receiver 参数；
  - 保持隐式 `it` 的既有协议，并兼容 `receiver + it` 同时存在的 lambda。

### 本轮关键进展
- 已完成 LLVM closure codegen 修复：
  - receiver lambda 的隐式 `this` 不再错误进入 capture env；
  - closure body 入口会把 `this` 绑定到 LLVM receiver 参数；
  - 保持隐式 `it` 语义不变，并支持 `receiver + it` 同时存在的 lambda。
- 已补充 / 更新回归：
  - `lambda_multi_param_type_is_propagated_from_expected`
  - `lambda_without_expected_explicit_params_basic`
  - `kotlin_scope_functions_basic`
  - `receiver_function_value_call_basic`
  - `receiver_lambda_this_shadows_outer_this`
- `receiver_lambda_this_shadows_outer_this` fixture 已收紧为只验证 `T4002` 目标语义：
  - 外层调用显式写成 `this.applyTo(...)`；
  - 类声明写成仓库现有构造语法 `class Box()`；
  - golden stdout 已更新为 `99`。

### 当前验证结果
- `cargo fmt --all`：通过
- `cargo build -p scoop`：通过
- `target/debug/scoop test --fixtures target/t4002-fixtures/infer`：`fixtures: ok (4)`
- `target/debug/scoop test --fixtures target/t4002-fixtures/run-pass`：`fixtures: ok (4)`
- `target/debug/scoop test --fixtures tests/fixtures/typecheck`：`fixtures: ok (326)`
- `cargo test --all`：通过
- `cargo clippy --all-targets -- -D warnings`：通过

### 收尾步骤
- 更新 `TODO.md` / `PLAN.md`，将 `T4002` 标记为完成并记录验证命令。
- 检查工作区 diff 后，使用 `[T4002] ...` 风格提交本轮改动，然后停止。
