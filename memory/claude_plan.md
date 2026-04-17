# 当前执行计划

## 说明

用户要求先把“思考过程和执行计划”写入此文件。这里记录的是可审计的决策摘要、检查步骤、执行计划和后续进展，不包含不可验证的冗长草稿式推理。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交说明中是否提到已知遗留问题、待修复问题或未完成事项。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，确认现有计划、任务依赖和编号约定。
4. 如果首个未完成任务过大，先把任务拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本轮只执行拆分后的第一个子任务。
5. 实现目标任务，同时检查过程中暴露的任何规范偏差或缺失能力；如发现阻塞项，按用户要求先在 `TODO.md`/`PLAN.md` 中显式建模依赖，再停止。
6. 运行与改动相关的测试，并尽量满足：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 以及该任务对应的更小范围测试
7. 更新文档状态：
   - 在 `TODO.md` 中标记已完成任务
   - 在 `PLAN.md` 中同步当前状态
   - 在本文件中补充执行记录、变更点和测试结果
8. 生成一次 Git 提交，提交信息采用仓库约定格式。
9. 停止，不继续处理下一个任务。

## 当前假设

- 仓库中存在 `TODO.md`、`PLAN.md`，且任务顺序可直接反映优先级。
- `memory/` 目录允许新增记录文件。
- 如遇到用户未提及但由最新提交或测试暴露的真实问题，这些问题在本轮同样属于范围内。

## 待补充

## 最新检查结果

- 最新提交：`94c68b13bf353d9f6cefa181008c7bc8f0256e4d`
- 提交说明：`[T3014R] Add prerequisite for same-op multi-arm dispatch`
- 结论：最新提交没有额外隐藏修复项，但明确点名了一个必须先修的既有缺口：统一 handler dispatch 仍把同一 `op_fqn` 的多个 arm 静默收缩成首个 arm。

## 当前目标任务

- `TODO.md` 中第一个未完成任务：`T3014a [TODO] 补齐同一 op_fqn 下多 arm 的 unified handler dispatch 合同`
- 当前判断：任务虽然涉及多层 plumbing，但边界清晰，暂不再拆分 `TODO.md` / `PLAN.md`。

## 已确认的现象

- 当前 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 在 handle dispatch 命中某个 `dispatch_entry` 后直接取 `dispatch_entry.arms().first()`。
- 复现实验（临时文件，不入库）：
  - 两个 `Raise.raise` arm，binder 分别为 `Sub` 与 `Any`
  - `runSub()` 触发 `Raise<Sub>`，预期命中第一个 arm，实际输出 `1`
  - `runBase()` 触发 `Raise<Base>`，预期命中第二个 arm，但当前实际也输出 `1`
- 这证明当前生产 dispatch 不是只“丢掉后续 arm metadata”，而是确实会把不同 handled-effect instance 的 same-op dispatch 错派到第一个 arm。

## 实现方案（本轮）

1. 给 typecheck / AST side table / HIR 补齐 effect-instance 元数据：
   - direct perform/await/join 等可形成 perform-slot 的表达式，要能把“performed effect 实例类型”写到 HIR；
   - handle arm 也要把 typecheck 推导出的“handled effect 实例类型”写回到 HIR，而不是只保留语法层的 `Effect` 路径。
2. 给 lowering 产物补齐 dispatch 所需的类型世界元数据：
   - nominal kind（至少要知道哪些 nominal 是 `effect`）
   - direct supertypes
   - declaration-site variances
   这样 LLVM codegen 可以在不重新依赖 typecheck `TypeEnv` 的前提下，复用与 typecheck 一致的“handled 是否可匹配 performed”判断。
3. 给 runtime perform slot 增加 effect-instance key：
   - perform 写 slot 时除了 `op_tag + payload` 外，再写一个稳定的 effect-instance key；
   - handle dispatch 读取 `op_tag` 后，还要读取这个 key。
4. 改写 unified handler dispatch：
   - 不再对 `dispatch_entry.arms()` 取首个 arm；
   - 对同一 `op_fqn` 下的 arms 按源码顺序做匹配；
   - 匹配依据不是源码形状，而是“performed effect instance 是否被该 handled effect 接住”的编译期合同。
5. 加回归：
   - 增加一个最小 run-pass fixture，覆盖 same-op multi-arm（`Raise.raise(Sub)` vs `Raise.raise(Any)`）；
   - 补一个 LLVM IR / emitter 定向测试，锁定 dispatch 路径已读取 effect-instance key，而不是继续只看 `op_tag`。

## 风险与注意点

- parser 当前对 handle arm head 不支持显式 `Raise<T>.raise(...)` 语法，因此 same-op multi-arm 只能通过 binder 类型推导出不同 handled-effect instance；实现必须保留这一点。
- 不能只修 direct perform；callee suspend / resumed body / outward propagation 最终都要走同一套 perform-slot metadata + handle dispatch。

## 待补充

- 具体修改文件列表
- 测试与提交结果
## 2026-04-17 接手续做

### 当前判断
- 延续上一轮已确认的目标，继续完成 `T3014a`，不展开到后续任务。
- 当前首要风险不是测试本身，而是 `ExprKind::Perform` 新字段与 effect-instance dispatch ABI 尚未全量接通，预计会先出现编译错误。
- same-op 多 arm 的正确修复必须闭环到 HIR、LLVM codegen、runtime ABI 和回归测试，不能只改单点分支选择逻辑。

### 本轮执行计划
1. 先扫描当前工作区和编译错误，补齐 `effect_ty` 引入后的结构体、pattern match 与构造点适配。
2. 在 runtime ABI 中加入 `effect_instance_key`，同步 LLVM runtime symbol / ABI 描述与所有 perform 写入调用点。
3. 在线程状态机 dispatch 侧把 `handled effect_ty` 元数据贯通到 unified arm，并基于 performed effect instance 做精确匹配。
4. 增加最小但足够的回归测试，覆盖 same-op 多 arm 在更具体与更泛 effect instance 下的正确分派。
5. 运行 `cargo fmt`、相关测试、`cargo clippy --all-targets -- -D warnings`；若通过，则更新 `TODO.md`、`PLAN.md`、本文件并提交。

## 进展更新

### 已完成
- `ExprKind::Perform.effect_ty` 已从 HIR 接到 LLVM `codegen_perform_expr`、状态机 `emit_perform_op` 和 direct perform 写槽路径。
- `MainCodegen` 已新增：
  - `effect_instance_key(effect_ty)`
  - `handled_effect_matches_performed(handled, performed)`（按 handle arm 语义使用“handled 可赋给 performed”方向）
  - `matching_effect_instance_keys_for_handled_effect(op_fqn, handled)`
- unified state machine 的 arm metadata 已贯通 `effect_ty`：
  - `ArmPlan.effect_ty`
  - `HandleSegmentArmBody.effect_ty`
  - `UnifiedArm.effect_ty`
- handle dispatch 主路径已改成：
  - 先读 `op_tag`
  - 再读 `effect_instance_key`
  - 在同一 `dispatch_entry` 内按源码顺序逐 arm 比较匹配 key 集
  - 不再出现 `dispatch_entry.arms().first()` 的主路径收缩
- LLVM runtime ABI 声明层已改成带 `effect_instance_key`：
  - `write_u64(op_tag, effect_instance_key, value)`
  - `write_u64_with_gc_ref(op_tag, effect_instance_key, word0, gc_ref)`
  - `write_u64_2(op_tag, effect_instance_key, word0, word1)`
  - `read_effect_instance_key()`
- runtime C perform-slot ABI 已补齐 `effect_instance_key` 字段与 getter/setter 签名。
- `cargo check -p scoopc --features llvm` 当前已重新通过。

### 当前未完成
- 需要同步 sysroot/fixture/LLVM 测试到新 ABI。
- 需要新增 same-op 多 arm 的 run-pass 回归和一个 emitter/IR 定向测试。
- 需要跑 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`，并根据结果继续收口。
## 2026-04-17 续做 T3014a

### 当前判断
- 最新提交已经检查过，没有新的额外 pre-existing issue 需要先修；当前阻塞仍然是 `T3014a` 自身未完成的 `Raise` 路径问题。
- 首个未完成任务仍然是 `T3014a [TODO] 补齐同一 op_fqn 下多 arm 的 unified handler dispatch 合同`，这一轮只完成它，不推进下一个任务。
- 上一轮已经把同一 `op_fqn` 多 arm dispatch 的大部分链路接通；剩余高概率问题是某些 `Raise.raise(...)` perform 在 lowering 或 codegen 中没有携带可编码的具体 effect instance，导致 emitter 读 `effect_instance_key` 时失败。

### 本轮执行计划
1. 先运行新增的定向测试，确认 `Raise` helper 的 typed lowering 是否保留了 performed effect instance。
2. 如果 lowering 测试失败，优先检查 `crates/scoopc/src/typecheck/expr/call.rs` 和 `crates/scoopc/src/hir/lower/expr.rs`，修复 `Perform.effect_ty` 的来源。
3. 如果 lowering 测试通过，检查 `crates/scoopc/src/llvm/codegen/mod.rs` 中 effect instance key 的收集与编码逻辑，修复无法识别 `Raise<...>` 实例的问题。
4. 修复后重跑与本任务直接相关的 LLVM IR 测试和端到端 fixture，确认：
   - IR 会读取 `effect_instance_key`
   - runtime slot ABI 仍正确
   - 同一 `op_fqn` 下多 arm dispatch 会按 effect instance 命中正确 arm
5. 在定向验证通过后，运行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 做收尾验证。
6. 更新 `TODO.md`、`PLAN.md`、本计划文件，勾掉 `T3014a` 并记录验证结果。
7. 提交一次 git commit，commit message 使用 `[T3014a] ...`，然后停止。

### 执行约束
- 不做 workaround；如果发现是新的规范缺口或前置缺失，必须先把新任务写入 `TODO.md`/`PLAN.md`，提交后停止。
- 不回滚他人改动；修改文件时继续使用 `apply_patch`。
- 本轮必须持续更新这个文件，至少在“定位结论明确”“修复完成”“验证完成”三个节点补充进展。

### 进展更新：定位结论明确
- 已运行 `cargo test -p scoopc --features llvm typed_lowering_preserves_raise_helper_performed_effect_instance`，测试通过。
- 结论：`Raise` helper 的 typed lowering 已经保留了具体 performed effect instance，当前问题不在 typecheck / HIR lowering。
- 下一步聚焦 `crates/scoopc/src/llvm/codegen/mod.rs` 的 `effect_instance_key()` 与已知 effect instance 收集逻辑，排查为什么 `Raise<...>` 在 codegen 阶段仍被判定为“无法编码的 effect instance key”。

### 进展更新：端到端验证发现运行时行为仍不对
- `cargo test -p scoopc --features llvm same_op_multi_arm_dispatch_ir_reads_effect_instance_key`、`effect_runtime_intrinsics_are_emitted_as_symbol_calls`、`multi_dispatch_handle_ir_registers_every_op_tag_on_handler_stack` 当前都通过。
- `tests/fixtures/run-pass/effect_runtime_slot_abi_basic.scoop` 直接执行二进制返回 `48`，与 `EXPECT-EXIT: 48` 一致。
- 但 `tests/fixtures/run-pass/effect_same_op_multi_arm_dispatch_effect_instance.scoop` 直接执行二进制返回 `0`，而不是期望的 `23`。这说明“IR 已读 `effect_instance_key`”并不等于“production dispatch 已按 effect instance 命中正确 arm”；仍需继续修运行时/dispatch 行为，当前任务不能收口。

### 进展更新：修复完成并进入全量收尾
- 已修复 `matching_effect_instance_keys_for_handled_effect()` 错把 `op_fqn` 当作 effect FQN 查候选集合的问题；现在按 handled effect 的 nominal FQN（必要时才回退从 `op_fqn` 反推）收集 effect-instance keys。
- 已收紧 IR 回归：`same_op_multi_arm_dispatch_ir_reads_effect_instance_key` 现在不仅要求读 key，还要求真的生成 `arm_*_effect_instance_match_*` 比较。
- 修复后重新验证：
  - `tests/fixtures/run-pass/effect_same_op_multi_arm_dispatch_effect_instance.scoop` 直接执行返回 `23`
  - `tests/fixtures/run-pass/effect_runtime_slot_abi_basic.scoop` 直接执行返回 `48`
  - 三个 LLVM 定向测试通过
- 全量 `cargo test --all` 暴露一个连带问题：`crates/scoop_runtime/tests/effect_tls.rs` 仍按旧 perform-slot ABI 断言，需把 runtime 测试同步到新增的 `effect_instance_key` 参数顺序后再继续收尾。
- runtime ABI 测试已同步并通过，但重新跑 `cargo test --all` 后又暴露一个剩余黄金文件断言：`hir_fixture_handle_perform_golden` 仍按旧 `ExprKind::Perform` 结构比较，缺少新增的 `effect_ty` 字段。需要把这个 golden 更新到当前 HIR 结构后再完成最终收尾。

### 进展更新：验证完成
- 已同步 `tests/fixtures/hir/handle_perform.hir`，补上 `ExprKind::Perform { effect_ty, .. }` 的 golden 结构。
- 最终验证结果：
  - `cargo test --all` 通过
  - `cargo clippy --all-targets -- -D warnings` 通过
  - `tests/fixtures/run-pass/effect_runtime_slot_abi_basic.scoop` 直接执行退出码为 `48`
  - `tests/fixtures/run-pass/effect_same_op_multi_arm_dispatch_effect_instance.scoop` 直接执行退出码为 `23`
- 文档状态已同步：`TODO.md` 把 `T3014a` 标记为完成，`PLAN.md` 已把当前执行顺序推进到 `T3014R`。
- 下一步只剩整理 git 状态并提交本轮唯一 commit，commit message 使用 `[T3014a] ...`。
