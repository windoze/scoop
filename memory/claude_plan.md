# 当前执行计划

## 约束说明

- 本文件记录的是可审计的执行计划、判断依据摘要、进度与变更记录，不包含未整理的内部推理草稿。
- 本轮目标是：先处理最新提交中提到的任何既有问题；若无，则读取 `TODO.md`，完成首个未完成任务，然后测试、更新计划与任务状态、提交 git，最后停止。
- 遇到任何既有缺陷、规格不一致、实现边界缺口或测试回归，都必须立即转为当前范围内问题处理；若无法当场修复，则要在 `TODO.md` 中插入前置任务并停止。

## 初始执行步骤

1. 检查最新一次提交信息，确认是否显式提到需要先修复的既有问题。
2. 查看工作区状态，避免误覆盖已有改动。
3. 读取 `TODO.md`、`PLAN.md` 与必要的项目说明，识别第一个未完成任务及其上下文。
4. 判断该任务是否足够小且可直接完成。
5. 若任务过大，则先把任务拆分为更小子任务，并更新 `TODO.md` 与 `PLAN.md`；本轮只执行拆分后的第一个子任务。
6. 实现当前目标任务，同时持续检查是否暴露出任何既有问题。
7. 运行与改动相关的测试；若需要，补充或修正测试，直到相关测试通过。
8. 运行必要的质量检查，至少覆盖本次修改涉及范围；若可行，执行 `cargo fmt`、相关测试与 `cargo clippy --all-targets -- -D warnings`。
9. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态或阻塞关系调整。
10. 使用清晰的提交信息提交本轮变更，然后停止，不继续下一个任务。

## 风险与判定标准

- 如果“首个未完成任务”依赖尚未实现或存在缺陷的语言特性、运行时能力、诊断行为或标准库能力，则不能绕过，必须先修复或把修复任务前置到 `TODO.md`。
- 如果测试或探查过程中发现已有问题，即使不在当前任务原始描述中，也必须先处理。
- 如果工作区存在用户已有改动，需要在不回退它们的前提下完成本轮工作。

## 进度记录

- 2026-04-25: 已创建本计划文件，下一步检查最新提交与任务列表。
- 2026-04-25: 已检查最新提交 `d4a2cb36cb0ffb764631570607429c8881cb8ade`，提交主题为 `[T4015a1] Bind non-generic const fun calls to typechecked targets`，未发现提交信息中额外声明需先修复的既有问题。
- 2026-04-25: 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务为 `T4015a2`：支持 generic `const fun` 的实例化与 type-substitution，并接入与 non-generic 相同的调用绑定主线。

## 当前细化计划（T4015a2）

1. 读取 `TODO.md` / `PLAN.md` 中 `T4015a2` 的详细说明与依赖，确认验收标准。
2. 盘点 `T4015a1` 相关代码路径：parser、resolve/typecheck、comptime evaluator/interpreter、相关测试。
3. 识别 generic `const fun` 当前卡点：
   - 调用绑定是否只能处理 non-generic 目标；
   - comptime 调用时是否缺少 type 参数实例化；
   - interpreter/evaluator 是否拒绝带类型参数的 const fun；
   - 反射 intrinsic 或显式类型实参是否存在额外阻塞。
4. 判断 `T4015a2` 是否能在单轮内完整收口。
   - 若可以：直接实现、补测试、跑验证。
   - 若不可以：把任务拆分为更小的前置子任务，更新 `TODO.md` 与 `PLAN.md`，本轮只执行第一个子任务。
5. 在实现过程中，若发现任何既有 bug / 规格不一致 / 边界缺口，会立即改为优先修复或前插 TODO 任务。

## 实施结果摘要（T4015a2）

- 已在 `crates/scoopc/src/comptime/interpreter.rs` 完成 generic `const fun` 实例化主线接入：
  - `ConstInterpreter` 现在持有 compilation-unit typecheck 产出的 `TypeStore`；
  - 新增活动类型实参作用域，用于在嵌套 `const fun` 调用中解析当前实例化后的类型参数；
  - `call_bound_const_fun(...)` 会消费 `TopLevelFunCallBinding.type_args`，并把外层类型实参递归替换到当前 generic `const fun` 实例；
  - `eval_fun_call(...)` 不再直接拒绝带 generic type params 的 `const fun`，而是走实例化后的统一调用路径；
  - `nameOf<T>()` / `fieldsOf<T>()` / `variantsOf<T>()` / `superTypesOf<T>()` / `annotationsOf<T>()` 等 reflection intrinsic 现可读取实例化后的真实类型；
  - 参数/返回类型强制中的 float coercion 也已兼容“类型参数实例化为 `Float32` / `Float64`”的场景。
- 已同步更新 `crates/scoopc/src/comptime/mod.rs` 的模块注释说明。
- 已新增/更新测试：
  - `crates/scoopc/src/comptime/tests.rs` 增加 generic `const fun` 单测；
  - 新增 `tests/fixtures/comptime/generic_const_fun_instantiation_basic.scoop`；
  - 新增 `tests/fixtures/comptime/generic_const_fun_instantiation_basic.comptime`。
- 已更新任务与问题跟踪：
  - `ISSUES.md`：收窄 issue 12 中关于 generic `const fun` 未接通的描述；
  - `TODO.md`：已将 `T4015a` / `T4015a2` 标记为完成；
  - `PLAN.md`：已把下一步推进到 `T4015b`。

## 验证记录

- 已执行 `cargo fmt`。
- 已执行 `cargo test -p scoopc const_eval_ -- --nocapture`。
- 已执行 `cargo run -p scoop -- test --fixtures tests/fixtures/comptime`。
- 已执行 `cargo run -p scoop -- test`，结果为 `fixtures: ok (1203)`。
- 已执行 `cargo test --all`。
- 已执行 `cargo clippy --all-targets -- -D warnings`。
- 以上验证均已通过，当前没有发现新的需前插处理的既有问题。

## 当前收尾计划

1. 复核 `git status`，确认只提交本轮相关文件，不包含用户已有的 `run_agent.sh` 改动。
2. 提交本轮变更，提交信息使用 `[T4015a2] Support generic const fun instantiation in comptime`。
3. 提交后立即停止，不继续执行 `T4015b`。

## 最新进度记录

- 2026-04-25: 已完成 `T4015a2` 实现、测试与文档更新；当前仅剩选择性暂存相关文件并提交。
- 2026-04-25: 已确认暂存区只包含 `T4015a2` 相关 9 个文件，用户已有的 `run_agent.sh` 改动未被纳入提交范围。
