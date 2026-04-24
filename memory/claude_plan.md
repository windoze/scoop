# 本轮执行计划（初始）

## 目标

按仓库约定执行一次最小完整迭代：

1. 检查最新提交是否提到已有问题；若提到，优先修复。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 判断该任务是否需要拆分；若需要，先更新 `PLAN.md` 与 `TODO.md`。
4. 只完成当前应执行的第一个任务。
5. 运行相关验证，修复执行过程中发现的已有问题。
6. 更新 `TODO.md` / `PLAN.md` / 本文件。
7. 提交 Git commit，然后停止。

## 决策依据摘要

- 必须先处理“最新提交中提到的已有问题”。
- 任何在检查、测试、实现过程中发现的既有缺陷，都属于当前范围，不能绕过。
- 不能一次做多个任务；若当前任务被前置问题阻塞，必须先把阻塞问题写回 `TODO.md`/`PLAN.md`，提交后停止。
- 需要在执行过程中持续维护本文件，记录计划变化与关键进展。

## 具体步骤

1. 查看最新一次提交信息与改动摘要，确认是否存在显式提及的待修复问题。
2. 查看工作区状态，避免误覆盖用户已有修改。
3. 读取 `TODO.md` 与 `PLAN.md`，确认第一个未完成任务及其上下文。
4. 若任务过大：
   - 拆成更小子任务；
   - 更新 `PLAN.md`；
   - 在 `TODO.md` 中重排并把当前可执行的第一个子任务置顶到未完成位置。
5. 阅读相关代码与测试，定位实现点。
6. 修改代码并补充/更新测试。
7. 运行最小充分验证；如果该改动影响面明确，再扩展到相关测试与质量检查。
8. 更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md` 记录完成情况。
9. 使用清晰提交信息创建一次 commit。
10. 停止，不继续下一个任务。

## 风险检查点

- 若发现规格不匹配、实现边界缺失、已有回避性实现或测试依赖 workaround，立即转为前置任务处理。
- 若 `PROMPT.md` 在过程中发生变化，需要随提交一并纳入。
- 不回退或覆盖与当前任务无关的现有修改。

## 当前轮次定位

- 已检查最新提交：`e5998551 [T4017b] Gate ordinary TLS checks by outward-effect analysis`。
- 该提交标题未显式声明仍待修复的既有 issue；当前未发现必须先于 `TODO.md` 顺序处理的新前置项。
- 已定位 `TODO.md` 首个未完成条目为 `T4017c`：在 compiler/runtime contract 中引入显式 `EffectCtx` / `EffectOutcome` / `EffectSignal` 抽象，并停止新增任何依赖 effect TLS 语义的路径。

## T4017c 执行计划

1. 阅读 `CONTINUATION.md`、`PLAN.md`、`TODO.md` 与实现中 effect/continuation 相关模块，确认当前 TLS 语义边界与现有分析/ABI 接口。
2. 检查是否已有可复用的 `EffectCtx` / `EffectOutcome` / `EffectSignal` / `ValueTransport` 表示；如果缺失，则先在编译器与 runtime 合同层引入最小抽象，不直接跨越到完整 ABI 迁移。
3. 在不提前实现 `T4017d/e/f` 的前提下，让新的主线代码和注释统一消费这些抽象，并避免继续扩散“TLS 是唯一 source of truth”的表达。
4. 为 `T4017c` 增补或更新定向测试/回归，优先覆盖：
   - effect contract 抽象已经进入主线；
   - 新路径不再以 TLS 语义命名内部协议；
   - 后续 `T4017d/e` 所需入口已稳定存在。
5. 运行最小充分测试；若过程中暴露既有问题，优先修复或写回前置任务。
6. 完成后更新 `TODO.md`、`PLAN.md` 与本文件，并创建本轮 commit。

## 当前进展

- 已完成：初始计划写入、最新提交检查、工作区检查、`TODO.md` / `PLAN.md` 首轮读取、`T4017c` 范围确认、实现、回归修复、状态文件更新准备。

## T4017c 实际完成内容

1. 编译器 contract 层：
   - 新增 `crates/scoopc/src/llvm/codegen/effect/contract.rs`。
   - 将 `ValueTransport` / `EffectSignal` / `EffectOutcome` 明确成 effect codegen helper。
   - ordinary propagation check、`perform` lowering、`Continuation.resume(...)` active fallback、handler dispatch 改为经由这层 helper 读写 current effect outcome/signal。
2. LLVM runtime ABI glue：
   - 在 `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 中补 `ScoopEffectCtx` / `ScoopValueTransport` / `ScoopEffectSignal` / `ScoopEffectOutcome` 的 struct type。
   - continuation 注释同步改成“captured handler stack top 在语义上代表 captured `EffectCtx.handler_top`”。
3. C runtime 内部表示：
   - 在 `runtime/c/scoop_runtime.c` 中补同名内部结构与 helper。
   - runtime-originated propagate/clear path 改经由 `ScoopEffectOutcome` helper。
   - continuation alloc/resume 改为围绕显式 `ScoopEffectCtx` 组织 captured/restored handler context 的叙事。
4. 回归测试：
   - 新增 LLVM 回归 `effect_contract_struct_types_are_registered_for_effect_codegen`。
   - 同步修正 3 处因 contract 命名收口导致的 LLVM 单测断言。

## 验证结果

- `cargo fmt --check`
- `cargo test -p scoopc --features llvm effect_contract_struct_types_are_registered_for_effect_codegen`
- `cargo test -p scoop_runtime --test effect_tls`
- `cargo test -p scoop_runtime --test continuation_one_shot continuation_double_resume_uses_shared_runtime_error_transport_contract`
- `cargo run -p scoop -- test`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`

以上均已通过。

## 收尾状态

- `T4017c` 可直接标记完成。
- 未发现需要前插到 `TODO.md` 的新 blocker。
- 下一轮应从 `T4017d` 开始。
