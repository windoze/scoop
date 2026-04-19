# 执行计划记录

## 说明

用户要求先写入“完整思考过程”。我不会记录不可公开的原始内部推理，但会在此文件中持续维护一份充分详细、可审计的执行计划、决策依据、关键检查点与进度更新，供后续检查。

## 初始计划

1. 查看最近一次提交的提交信息与改动，确认是否提到了需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与 `TODO.md` 是否一致。
4. 评估首个未完成任务的复杂度：
   - 如果任务足够明确且可在一次迭代内完成，直接实施。
   - 如果任务过大或存在隐含前置依赖，则先拆分任务，更新 `PLAN.md` 与 `TODO.md`，并只执行拆分后的第一个子任务。
5. 实现任务，同时检查是否暴露出新的规范不匹配、缺失特性或已有缺陷：
   - 若发现阻塞性缺口，不做规避实现。
   - 将缺口转化为更靠前的 `TODO.md` 任务，更新 `PLAN.md` 说明依赖关系，然后提交并停止。
6. 运行与本次改动相关的测试，以及必要的质量检查：
   - 至少运行受影响范围测试。
   - 若改动较广，补充运行 `cargo test --all`。
   - 按要求运行 `cargo clippy --all-targets -- -D warnings`，确保无警告。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中将当前已完成任务标记完成。
   - 在 `PLAN.md` 中同步记录当前状态与后续影响。
   - 如有必要，补充 `README.md` 或代码注释。
8. 使用清晰的提交信息提交本次变更。
9. 完成一个任务后立即停止，不继续处理下一个任务。

## 执行记录

- 已查看最近一次提交：
  - `HEAD` 为 `63bc23bc3ad5c45ae593d9dd6a71788442a4b69f`
  - 提交信息：`[T4005T] 收口顶层 callable value 调用主线`
  - 提交信息本身未显式声明新的待修既有 issue。
- 已读取 `TODO.md` / `PLAN.md`：
  - 当前第一个未完成任务为 `T4005SR [TODO] Review：确认 callable-value 主线已覆盖 pattern binder`。
  - `PLAN.md` 与 `TODO.md` 一致，下一项均指向 `T4005SR`。

## 当前任务：T4005SR

### 任务性质

这是 review 任务，目标不是盲目新增特性，而是确认上一轮 `T4005S/T4005T` 收口后的 callable-value 主线是否真的统一覆盖以下场景：

1. 普通局部函数值调用。
2. 局部 pattern binder 引入的函数值调用。
3. `when` pattern binder 引入的函数值调用。
4. 顶层命名 `val` 函数值调用。
5. 顶层 pattern binder 函数值调用。
6. 顶层 `FunPtr` direct call。

### review 执行计划

1. 阅读上一轮主要改动文件与回归 fixture，理解 typecheck / HIR / LLVM 当前主线。
2. 设计扩展 probe，优先覆盖容易遗漏的统一性风险：
   - receiver function value / receiver `FunPtr`；
   - named args 与 direct call 的组合；
   - callable value 出现在 pattern binder 后再次转发、返回或嵌套调用；
   - 顶层 callable value 在其它顶层 initializer 中被调用。
3. 运行定向 `scoop run` / `scoop test` / 必要的 Rust 单测，确认是否存在新的裂缝。
4. 若发现问题：
   - 判断是否可以在本轮 review 内直接修复并补回归；
   - 若存在更基础 blocker，则按要求先更新 `TODO.md` / `PLAN.md`，记录依赖后停止。
5. 若未发现新 blocker：
   - 更新 `TODO.md` 将 `T4005SR` 标记完成，并写明 review 结论；
   - 更新 `PLAN.md` 记录本轮复审结论与下一项任务；
   - 运行最终验证；
   - 提交并停止。

### 当前发现

已通过定向 probe 发现一个新的真实裂缝，且它直接影响 `T4005SR` 的 review 结论：

- 场景：顶层 pattern binder 的 tuple initializer 中放置 receiver lambda。
- 最小 probe：
  - `val (topPatternF, topPatternBase): (String.(Int) -> Int, Int) = ({ n: Int -> this.length() + n + 1 }, 3)`
- 当前结果：
  - `cargo run -p scoop -- run /tmp/t4005sr-probe.btB9bn/top_level_receiver_lambda_pattern_probe.scoop`
  - 报错：`scoop::typecheck::unknown_local_value_type`，信息为“无法获取局部绑定的类型：this”。
- 对照验证：
  - 顶层命名 receiver function value `val topNamed: String.(Int) -> Int = { ... }` 可正常运行；
  - 顶层 `@ThreadLocal var` 上的 `FunPtr` direct call 也可运行；
  - 因此问题收敛到“expected tuple type 未向 tuple 元素传播 expected element type”，不是泛化的顶层 callable-value 全面失效。

### 修复方向

1. 在 typecheck `infer_expr_type_in_expected_context` 中为 tuple literal 增加 expected-context 传播。
2. 让 tuple 元素在存在 expected tuple type 时逐元素调用 `infer_in_expected(...)`，从而把 receiver function type 正确下传给 closure / receiver lambda。
3. 补一条 review 级 run-pass 回归，覆盖：
   - 顶层命名 receiver function value；
   - 顶层 pattern binder 中的 receiver function value；
   - 局部 destructuring；
   - `when` binder；
   - 顶层 `FunPtr`；
   - 上述调用均使用 receiver + named args，验证主线确实统一。

### 已完成事项

1. 已在 `crates/scoopc/src/typecheck/expr/infer.rs` 中补齐 tuple literal 的 expected-context 传播：
   - 当外层存在 expected tuple type，tuple 元素会逐个调用 `infer_in_expected(...)`；
   - receiver function type 因此能够正确下传到 tuple 元素中的 lambda / receiver lambda。
2. 已新增 review 回归：
   - `tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`
   - `tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.stdout`
3. 已更新 `TODO.md` / `PLAN.md`：
   - `T4005SR` 已标记完成；
   - 已记录本轮 review 发现并修复的 tuple expected-context 裂缝；
   - 下一项任务已切换为 `T4006`。

### 验证结果

- `cargo run -p scoop -- run tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`
  - 输出：`12 7 7 7 8 6 15 10`
- `cargo run -p scoop -- run tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`
  - 输出：`11 11 11 11 3 3`
- `cargo run -p scoop -- run tests/fixtures/run-pass/when_pattern_function_value_call_basic.scoop`
  - 输出：`7 12`
- `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - 结果：`fixtures: ok (329)`
- `cargo test --all`
  - 结果：通过
- `cargo clippy --all-targets -- -D warnings`
  - 结果：通过

### 收尾

- 当前工作树内容与本轮 `T4005SR` 相关。
- 下一步：提交本轮变更并停止，等待下一次调用处理 `T4006`。
