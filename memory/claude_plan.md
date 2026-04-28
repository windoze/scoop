# 本轮执行计划

## 约束说明

- 按用户要求，本文件用于记录本轮的执行计划、关键决策、进度更新与计划变更。
- 我不会在这里写出不可审计的隐含推理细节，但会完整记录可执行的步骤、依据、发现的问题与后续动作。
- 本轮目标是：先检查最新提交是否提到需先修复的既有问题；然后定位 `TODO.md` 中第一个未完成任务；只完成这一个任务（或在必要时先拆分/重排依赖），完成后测试、更新文档并提交 Git commit，然后停止。

## 初始步骤计划

1. 检查最新一次 Git 提交信息，确认是否显式提到需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务是否已有既定分解或依赖。
4. 结合相关代码、测试、规范与现状，判断该任务是否可以在本轮完整完成。
5. 如果任务过大或被既有缺陷阻塞：
   - 在 `TODO.md` 中把阻塞缺陷或必要前置项插到当前任务之前；
   - 必要时同步细化 `PLAN.md`；
   - 记录原因并提交后停止。
6. 如果任务可执行：
   - 实现任务；
   - 运行相关测试、`cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`（或与改动最相关的最小充分测试集，若失败则继续修复直到通过）；
   - 更新 `TODO.md` 和 `PLAN.md`；
   - 提交 Git commit；
   - 停止。

## 进度记录

- 已创建本计划文件，尚未开始仓库检查。
- 已检查 `git log -1 --stat --decorate=short`：
  - 最新提交为 `21712e248907ce17352126d7f819da684a5c3166 [T5000iR] Review effect middle-end boundary migration`；
  - 提交信息本身未显式提出需优先修复的新增既有问题。
- 已阅读 `TODO.md` / `PLAN.md`：
  - 当前第一条未完成任务是 `T5000j 扩展覆盖面，并继续跟踪 safepoint / mem2reg 方向`；
  - 该任务同时覆盖 `when/pattern`、operator-overload、higher-order/closure/object-init/top-level-init，以及 safepoint/root-pressure 跟踪，单轮过大，必须先拆分。
- 已完成的拆分依据收集：
  - `OPTIMIZATION.md` 明确点名“operator overload 目标确定仍发生在 codegen 阶段”，并指出这导致 `llvm/emit.rs` 仍需 eager inclusion struct member methods；
  - `crates/scoopc/src/llvm/emit.rs` 当前确实存在“把所有 struct member methods 补进 reachable 集”的 eager inclusion；
  - `crates/scoopc/src/llvm/codegen/mod.rs` 当前仍在 `codegen_binary(...)` 中现场决定 user-defined operator overload / `compareTo`；
  - `crates/scoopc/src/mir/lower.rs` 当前把普通 overloaded binary 仍降为 `Rvalue::Binary`，没有显式 direct-call target；
  - probing 还暴露了同一主题下的既有覆盖缺口：typecheck 已支持 unary operator overload，但 runtime/codegen 主线没有把它 materialize 到 direct-call 边界，因此不能把首个子任务狭义限定为“只修二元运算符”。
- 当前决策：
  - 已先把 `T5000j` 拆成围绕结构边界的子任务，并同步回写 `TODO.md` / `PLAN.md`；
  - probing 后又把 `T5000j1` 继续细分为 `T5000j1a` / `T5000j1b`：
    - `T5000j1a`：先处理 unary 与 arithmetic/bitwise/shifts operator overload 的 direct-call 主线；
    - `T5000j1b`：再单独处理 user-defined `compareTo` 比较与剩余 eager inclusion 删除；
  - 这样拆分的原因是：当前 HIR/MIR 整数字面量节点不承载可直接合成的 `0` 常量值，`compareTo` 比较不能和普通 operator overload 一样直接机械重写；
  - 本轮实际执行目标已更新为 `T5000j1a`。
  - 之后再分别处理 `T5000j1b`、pattern/when、更多 higher-order / init 覆盖，以及 safepoint/root-pressure 跟踪。

## 2026-04-28 T5000j1a 接手记录

- 已检查当前未提交改动：上一轮只修改了
  - `crates/scoopc/src/typecheck/expr/call.rs`
  - `crates/scoopc/src/typecheck/expr/ops.rs`
  - `PLAN.md`
  - `TODO.md`
  - `memory/claude_plan.md`
- 已确认服务端中断发生在 AI 执行阶段，而不是本地编译/测试阶段；当前需要基于这些未提交中间态继续完成 `T5000j1a`。
- 已完成的缺口定位：
  - 类型检查侧新增了 `record_member_operator_direct_call_binding(...)`，开始为 unary/binary operator overload 记录 `TopLevelFunCallBinding` 与 monomorph request；
  - 但 `infer_unary_expr_type(...)` 当前把 unary binding 记在 `operand.span`，而不是一元表达式自身的 span；若不修正，HIR lowering 无法在 `~expr` 节点上取回 direct-call target；
  - `crates/scoopc/src/hir/lower/expr.rs` 当前仍把 unary / binary operator site 直接降成 `ExprKind::Unary` / `ExprKind::Binary`，没有消费新的 direct-call binding；
  - `crates/scoopc/src/mir/lower.rs` 因此仍会为这些 site 生成 `Rvalue::Unary` / `Rvalue::Binary`，production MIR / reachability 主线还未真正接到 operator overload target；
  - `crates/scoopc/src/llvm/emit.rs` 仍保留“把所有 struct member methods eager inclusion 进 reachable 集”的老兜底逻辑，范围尚未缩到只剩 `compareTo`。
- 当前实施方案：
  1. 修正 unary operator overload 的 binding span，使其绑定到外层 unary 表达式；
  2. 在 `crates/scoopc/src/hir/lower/expr.rs` 中为 `~` 与 arithmetic/bitwise/shifts operator overload 增加显式 direct-call lowering，把这些节点改写成统一的顶层 `ExprKind::Call` 形状；
  3. 让改写路径继续复用已有 `materialized_top_level_fun_call_target_fqn(...)` / `TopLevelFunCallBinding`，保持 generic owner specialization 与 effect-row default eff-arg 主线不分叉；
  4. 缩小 `crates/scoopc/src/llvm/emit.rs` 的 eager inclusion，只为尚未迁出的 `compareTo` 比较路径保留最小范围；
  5. 增加/更新 HIR lowering 与 LLVM production regression，证明：
     - operator overload site 已改写为显式 direct call；
     - reachability 能只收集真正用到的 operator callee，而不会再把同 struct 的其它 member method 一起托底带入 IR；
  6. 运行格式化、相关测试、全量 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`，通过后更新 `TODO.md` / `PLAN.md` 并提交。

## 2026-04-28 T5000j1a 完成记录

- 代码已完成：
  - `crates/scoopc/src/typecheck/expr/infer.rs` / `ops.rs`
    - unary `~` operator overload 现在把 `TopLevelFunCallBinding` 绑定在外层一元表达式 span；
    - arithmetic/bitwise/shifts operator overload 继续统一记录 direct-call binding / monomorph request。
  - `crates/scoopc/src/hir/lower/expr.rs`
    - `~` 与 arithmetic/bitwise/shifts operator site 现会被改写成显式顶层 `ExprKind::Call`；
    - 改写路径复用已有 direct-call binding、owner specialization type args 与 eff-arg materialization 逻辑，没有新增 backend-only 分支。
  - `crates/scoopc/src/llvm/emit.rs`
    - operator-overload 兜底 eager inclusion 已缩到只剩 `compareTo` 比较路径；
    - `plus`/`inv`/`shl` 等不再靠“把整类 struct member methods 全部塞进 reachable 集”托底。
- 新增/更新回归：
  - `crates/scoopc/src/mir/materialize.rs`
    - 新测试验证 operator-overload binding / monomorph key 会保留 owner specialization 的 `Int` type arg 与非 `Pure` 默认 eff-arg。
  - `crates/scoopc/src/llvm/tests.rs`
    - 新测试验证 typed HIR 会把 `~` / `+` / `<<` lower 为 direct call；
    - production LLVM IR 只包含实际用到的 `Mask.inv` / `Mask.plus` / `Mask.shl`，不会把未使用的 `Mask.minus` 一起带入。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`
- 结论：
  - `T5000j1a` 已满足“typed HIR / generic MIR / reachability / production LLVM body emission 改走 explicit direct-call 主线”的验收要求；
  - 下一条待执行任务为 `T5000j1b`，负责 `compareTo` 比较 target 与剩余 eager inclusion 的最终清理。
