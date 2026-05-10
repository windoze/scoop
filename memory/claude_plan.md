# 执行计划

## 说明
- 不记录逐字的内部推理；这里保留可审计的执行摘要、决策依据和步骤计划。
- 本次目标是严格按 `TODO.md` 顺序完成第一个未标记为 `[DONE]` 的任务，然后停止。

## 初始计划
1. 读取 `TODO.md`，定位第一个未完成任务，并确认其要求、依赖、验证方式、完成记录格式。
2. 检查最近一次提交信息是否直接提到该任务相关的未完成事项；如果这是当前任务的直接组成部分或前置条件，则纳入本次处理范围。
3. 阅读与该任务直接相关的代码、测试、文档，仅收集完成当前任务所需的最小上下文，避免无关排查。
4. 判断当前任务是否可直接完整落地：
   - 若可以，按最小正确改动实现。
   - 若存在阻塞当前任务的真实缺口或规格不匹配，则先在 `TODO.md` 中加入最小必要前置任务并调整顺序，再停止。
5. 实现改动后，运行该任务要求的验证与必要的相关测试；若失败，立即修复并重新验证。
6. 更新文档：
   - 在 `TODO.md` 中把已完成任务标题改为 `[DONE]` 前缀，并补充完成记录。
   - 仅当阶段计划确实变化时才更新 `PLAN.md`。
   - 在本文件补充关键进展、计划变更与验证结果。
7. 按仓库约定创建一次 git 提交，提交信息包含任务编号。
8. 停止，不继续下一个任务。

## 进度记录
- 已创建本计划文件。
- 已读取 `TODO.md`，确认首个未完成任务为 `G0-T01：硬删除后的物理残余清场，恢复“最小一致破坏状态”`。
- 下一步：检查最近提交信息与当前工作区状态，确认是否存在与 `G0-T01` 直接相关的未完成续作或未提交恢复现场；随后仅阅读该任务直接涉及的代码与测试位置。
- 已检查最近提交：`5701ba08 [REFACTOR] Keep LLVM emit on the stage-owned MIR path`，未显式声明与 `G0-T01` 直接绑定的 unfinished issue。
- 已检查工作区：存在大量与 effect refactor 相关的未提交改动，视为当前现场的一部分，不回退；本次只在 `G0-T01` 直接涉及的位置上继续收口。
- 已完成定点阅读：
  - `runtime/c/scoop_runtime.c` 315-384 一带仍残留已删类型的 `_Static_assert`、handler-stack 注释与孤立类型定义；
  - `scoop_alloc` 在 `scoop_runtime.c` 中被先用后定义，缺少中性前置声明；
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/{value,body}.rs` 仍有少量“旧名字”测试断言文本；
  - `runtime/c/scoop_test.c`、`crates/scoopc/src/effect_facts/builder.rs`、`sysroot/core.scoop` 目标残余在当前工作区中已基本清理。
- 计划中的最小改动：
  1. 删除 `runtime/c/scoop_runtime.c` 的已删类型断言/注释/孤立 handler-stack 结构残余，并补回 `scoop_alloc` 前置声明；
  2. 把活跃测试中的旧名字断言替换成不依赖 legacy 名称的目标 surface 验证；
  3. 运行 grep、`cargo check -p scoop_runtime`、`cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`；
  4. 若验证通过，更新 `TODO.md` 完成记录并提交。
- 已完成代码编辑：
  - `runtime/c/scoop_runtime.c`：删除已删 `ScoopEffectPerformSlot` / `ScoopEffectCtx` / `ScoopValueTransport` / `ScoopEffectHandlerFrame` 残余断言与孤立 handler-stack 段落；补回 `scoop_alloc` 中性前置声明；
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`：把旧桥名字负向断言替换为对 `scoop_runtime_init` 的正向 IR 验证；
  - `crates/scoopc/src/llvm/codegen/effect_lowered/{value,body}.rs`：移除围绕旧桥名字的源码审计断言，保留其余 refactor 边界检查。
- 下一步：执行 grep 与编译/静态检查，确认剩余失败是否已切换为 TODO 预期的 architecture gap。
- 已补充清理：删除 `runtime/c/scoop_runtime.c` 中当前不再被任何路径引用的 GC stress / immix helper 尾巴，使 `scoop_runtime` 回到 warning-free 的最小一致状态。
- 验证结果：
  - 针对 `crates/scoopc/src`、`runtime/c`、`sysroot` 的旧 TLS/bridge 符号 grep：无命中；
  - `cargo check -p scoop_runtime`：通过；
  - `cargo clippy -p scoop_runtime --all-targets -- -D warnings`：通过；
  - `cargo check -p scoopc`：仍失败，但错误集中在 G1/G2/G4/G5 对应的缺失 helper / ABI / lowering contract，不再由物理残余主导。
- 已回写 `TODO.md`：`G0-T01` 标记为 `[DONE]`，并记录改动范围、核心决策、验证结果与消除的 gap。
- 已检查当前 worktree：除本次补做的清场外，还保留一批与同轮 hard-delete/refactor 直接相关的未提交改动（legacy effect/call 模块删除、验证面迁移、任务文档落地等）。按用户给定的“若为失败恢复现场则提交所有当前未提交文件”规则，本次提交将整体纳入这些现有改动，避免把 `G0-T01` 留在半提交状态。
- 下一步：按任务号创建一次覆盖当前未提交状态的提交，然后停止。
