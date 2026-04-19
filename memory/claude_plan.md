# 执行记录

## 说明

按要求先记录可审阅的高层计划、关键判断依据与执行步骤。此文件会在后续关键进展、计划调整、问题发现和任务完成时持续更新。

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在执行前发现最近一次提交提到的遗留问题，则先修复这些问题，再处理 `TODO.md` 任务。

## 初始执行计划

1. 检查最新一次 Git 提交的提交信息与改动，确认是否明确提到尚未修复的已知问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务的上下文、依赖和既有计划。
4. 如果该任务范围过大或存在明确前置依赖，先把它拆成可独立交付的小任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
5. 在实现前检查相关代码、测试和规格文档，确认没有以规避方式绕过语言/运行时缺陷。
6. 实现当前任务所需改动。
7. 运行与改动相关的测试；若任务影响范围较广，再补充运行更高层级的验证命令，包括格式化、测试和 `clippy`，直到无警告、无失败。
8. 更新文档与追踪文件：
   - 在 `TODO.md` 中将当前任务标记为完成，或在受阻时按依赖重排任务。
   - 在 `PLAN.md` 中记录当前状态、后续影响与任何新增依赖。
   - 在本文件中补充实际执行结果与变更原因。
9. 使用清晰的提交信息提交本轮所有改动。
10. 停止，不继续处理下一个任务。

## 风险与约束

- 不接受规避实现、临时兼容层、仅为夹具通过而做的 hack。
- 如果发现规格与实现不一致，必须先把缺口变成 `TODO.md` 中更靠前的任务，再停止于该轮。
- 不能回退或覆盖仓库中与本轮任务无关的已有改动。
- 需要确保编译、测试、lint 无警告。

## 当前进展

- 已检查最新提交 `a8a9f0e74d16b88ed700a240021328de8fe34ac5`：提交信息未显式声明仍待修复的遗留问题，因此无需先插入额外“修前一提交问题”的任务。
- 已读取 `TODO.md` / `PLAN.md`：当前顺序上的首个未完成任务为 `T4008c2`“打通 receiver effect op 的 perform / handler lowering / codegen”。
- 该任务目前看起来是一个完整但边界清晰的切片，暂不需要再拆分；不过在真正改动前仍需确认：
  1. typecheck 中对 receiver effect op 的 early gate 具体分布；
  2. handler arm binder / HIR 表示是否已默认假设“无 receiver”；
  3. LLVM perform payload 与 arm payload unpack 是否能直接复用已有多 payload transport 主线。

## 接下来要做的事

1. 读取 effect-op typecheck、handle arm typecheck、HIR lowering、LLVM effect codegen 的相关实现。
2. 做一个最小 receiver effect op probe，确认当前失败点与报错形态。
3. 按统一主线修改 typecheck / lowering / codegen。
4. 增加 parse / typecheck / run-pass 回归。
5. 跑定向验证，再跑全量 `cargo test --all`、`cargo run -q -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 实际执行结果

### 代码实现

- 已在 `crates/scoopc/src/typecheck/expr/call.rs` 移除 receiver effect op 的调用侧 early gate，并把 effect op receiver 统一降为显式第 0 个形参：
  - named/positional args 继续复用既有 `arg_mapping` 绑定；
  - effect instance 的类型实参推断可直接利用 receiver 实参参与约束。
- 已在 `crates/scoopc/src/typecheck/expr/infer.rs` 移除 handler arm 的 receiver early gate，并把 receiver 统一降为第 0 个 binder：
  - binder arity、类型注解校验与 handled-effect 推断继续复用已有多 binder 主线；
  - 没有新增 receiver 专用 handler 语义分叉。
- 已确认 HIR / LLVM 不需要新增 receiver 专用 lowering：
  - `Perform.args`、`EffectOpCallInfo { arg_mapping, payload_tuple_ty }`、`handle_payload_tuple_tys` 与 ordinary/state-machine perform transport 已能直接承接 receiver + payload 组合。

### 回归与文档

- 已新增 parse fixture：`tests/fixtures/parse/effect_op_receiver_decl_basic.scoop` + `.ast`，覆盖 effect op 声明中的 extension-style receiver AST 保留。
- 已新增 typecheck fixture：`tests/fixtures/typecheck/effect_receiver_op_call_and_handle_ok.scoop`，覆盖 receiver 作为第 0 个显式形参与第 0 个 binder 的调用/handler 校验。
- 已新增 run-pass fixture：`tests/fixtures/run-pass/effect_receiver_op_basic.scoop` + `.stdout`，覆盖 non-resuming / immediate-resume / escape-continuation 三条路径。
- 已同步更新：
  - `TODO.md`：将 `T4008c2` 标记为完成；
  - `PLAN.md`：记录本轮完成情况并把下一项推进到 `T4008c3`；
  - `ISSUES.md`：删除已过时的“多 effect type params / receiver effect op / escape continuation binder 仍未完成”描述，收窄到当前真正剩余的 surface gap。

### 执行中遇到的问题

- 首次跑全量 fixtures 时，新增的 typecheck fixture 因重复使用 `_` 作为局部名而触发“重复定义：_”。
- 这不是实现回归，而是 fixture 自身命名问题；已改为唯一局部名后，定向与全量验证恢复通过。

### 验证结果

- `cargo fmt --check`：通过。
- `cargo run -q -p scoop -- dump-ast tests/fixtures/parse/effect_op_receiver_decl_basic.scoop`：通过，AST golden 已同步。
- `cargo run -q -p scoop -- run tests/fixtures/run-pass/effect_receiver_op_basic.scoop`：stdout 符合预期，退出码 `30`。
- `cargo test --all`：通过。
- `cargo run -q -p scoop -- test`：通过，`fixtures: ok (1065)`。
- `cargo clippy --all-targets -- -D warnings`：通过。

## 本轮结论

- `T4008c2` 已完成，且未发现需要再向前插入的新 blocker。
- 下一轮应从 `T4008c3` 开始，继续收口 handler arm head 的 effect-op 绑定主线。
