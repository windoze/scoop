# 执行计划与进展记录

## 说明

按要求，我会在此文件中持续记录：
- 当前要处理的任务
- 执行步骤
- 关键判断与阻塞原因
- 已完成的实现、测试、文档与提交进度

出于能力与安全边界限制，这里不会记录逐字的内部思维过程，但会提供完整、可审阅的外部执行计划与决策摘要。

## 初始计划

1. 检查最新一次 Git 提交，确认提交说明中是否提到任何需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖和预期范围。
4. 判断该任务是否足够小且可在本次调用内完整完成：
   - 如果可完成，则直接实现。
   - 如果过大，则拆分为更小子任务，并更新 `PLAN.md` 与 `TODO.md`，随后执行拆分后的第一个子任务。
5. 实现任务所需代码修改，必要时补充或整理相关注释、测试、文档。
6. 运行相关验证：
   - 至少运行与改动直接相关的测试；
   - 如任务涉及公共行为或基础设施，补充运行更广泛验证；
   - 确保 `cargo clippy --all-targets -- -D warnings` 无警告（若当前任务范围允许）。
7. 更新任务跟踪文件：
   - 在 `TODO.md` 中标记本次完成的任务；
   - 在 `PLAN.md` 中记录当前状态与后续影响；
   - 在本文件中补充执行结果与测试结论。
8. 使用清晰的 Git 提交信息提交本次改动。
9. 完成一个任务后停止，不继续处理下一个任务。

## 进展

- 已创建本文件并写入初始计划。
- 已检查最新提交、`TODO.md`、`PLAN.md` 与工作树：
  - 最新提交为 `361fe19 [T3016] Connect handle return function propagation`。
  - 提交说明未额外挂出需要先单独修复的既有问题。
  - `TODO.md` 中首个未完成任务为 `T3016R`：复审 `STATE_TAG_FUNCTION_RETURNED` 是否真正接回 enclosing function return 合同。
- 当前实际执行目标：完成 `T3016R`，若复审中发现生产代码问题，则在本轮直接修复；若未发现问题，则更新任务文档并提交复审结论。

## 当前复审计划：T3016R

1. 审查 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中以下路径：
   - `ReturnFromFunction` terminator 的发射；
   - step function / dispatch loop 中的 `effect_function_return_context` 桥接；
   - `handle_done` 对 `STATE_TAG_FUNCTION_RETURNED` 的消费；
   - cleanup/finally 期间 `completion_tag` 的捕获与恢复。
2. 审查 `crates/scoopc/src/llvm/codegen/mod.rs` 中既有函数返回基础设施：
   - `return_context`
   - `finish_function_return_path()`
   - `emit_return()` 的调用关系
3. 确认 unified effect 主线没有新增 effect-only 返回旁路，也没有回流 flag-based unwind 式的函数返回处理。
4. 运行定向验证：
   - 与 handle 内 `return`、nested handle、finally 相关的 dedicated fixtures；
   - 至少一条与 planner / state machine 相关的定向测试；
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 如时间允许，复跑 `cargo run -p scoop --features llvm -- test`，确认首个停止点仍是已跟踪的 `T3017` stale expectation，而非新的生产回归。
5. 根据复审结果更新 `TODO.md`、`PLAN.md` 与本文件，然后提交。

## 当前结果

### 复审结论

- `T3016R` 在复审中发现了一个真实生产缺口，而不是“仅需记录无问题”的纯审计任务。
- 缺口内容：
  - `HandleStateOp::Return` 在 state-machine emitter 中重新求值 `return expr` 时，使用了 `expected = None`。
  - 这与普通 `return` 语句路径不一致；普通路径会按 enclosing function 的声明返回类型走 expected-context + coercion。
  - 直接后果是：当 `handle` 内的 `return` 需要发生返回值 coercion（例如 `Int -> Any` boxing）时，effect frame transport 写入的是未 coercion 的值，函数出口读回时会丢失真实返回对象。
- 已修复：
  - `HandleStateOp::Return` 现改为使用 `enclosing_function_return_ty()` 作为 expected context。
  - 这样 early-return payload 会在写入 effect transport slots 之前，先与普通 `return` 一样完成 coercion。

### 用于定位问题的临时验证

- 我创建了两个临时样例做对照验证：
  - 普通 `return 1` 返回到 `Any`
  - `handle` 内 `return 1` 返回到 `Any`
- 修复前，对照结果表明：
  - 普通路径在 GC 后仍保留 1 个 boxed live object。
  - `handle` 路径在 GC 后错误回落到 0，说明返回值没有按普通返回合同保活。
- 这些临时样例仅用于诊断，不会保留在最终提交中。

### 新增回归

- 已新增 dedicated run-pass fixture：
  - `tests/fixtures/run-pass/effect_handle_return_from_function_any_boxing.scoop`
  - `tests/fixtures/run-pass/effect_handle_return_from_function_any_boxing.stdout`
- 该用例锁定：
  - `handle` 内 `return 1`
  - enclosing function 返回类型为 `Any`
  - `Int -> Any` boxing 发生后，GC 之后 boxed object 仍然存活

### 已完成验证

- 定向运行新 fixture：通过
- 定向运行既有 T3016 fixtures：通过
  - `effect_handle_return_from_function_basic.scoop`
  - `effect_handle_return_from_function_finally.scoop`
  - `effect_handle_return_from_function_nested_handle.scoop`
- 定向运行 cleanup baseline：通过
  - `effect_handle_yield_and_step_finally.scoop`
- 定向单测：通过
  - `cargo test -p scoopc plan_and_segments_support_return_inside_handle_body_block_expression -- --nocapture`
- 全量验证：通过
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 复跑 LLVM fixture suite：
  - `cargo run -p scoop --features llvm -- test`
  - 结果仍只停在已跟踪的 `tests/fixtures/run-pass/effect_escape_continuation_async_executor_fifo.scoop` stale `EXPECT: fail`
  - 说明本轮没有引入新的更早生产回归

### 文档/任务状态

- 已在 `TODO.md` 中将 `T3016R` 标记为完成，并记录复审中发现的问题、修复内容与审查结论。
- 已在 `PLAN.md` 中写入本轮复审更新，并将下一项推进到 `T3017`。
- 本轮按要求只完成一个任务，到此停止。
