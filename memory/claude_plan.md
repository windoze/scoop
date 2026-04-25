# 本轮执行计划

## 任务理解

本轮目标是严格按照仓库流程推进一次最小闭环：

1. 先检查最新提交是否提到了尚未修复的问题；如果提到，优先修复该问题。
2. 阅读 `TODO.md`，找到第一个未完成任务。
3. 如果该任务过大，则先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
4. 对当前要执行的任务完成实现、测试、文档更新与提交。
5. 完成后立即停止，不继续做下一个任务。

## 执行约束

- 不接受规避实现边界的 workaround。
- 在探查、测试、实现过程中遇到任何既有缺陷、规格不一致、回归或实现缺口，都必须立即纳入当前范围。
- 如果发现阻塞当前任务的前置问题，必须先把该问题作为前置任务插入 `TODO.md`，更新 `PLAN.md`，提交后停止。
- 本轮必须维护 `memory/claude_plan.md`：在计划变化、发现关键问题、完成关键步骤时及时更新。

## 当前分步计划

1. 查看最新一次 Git 提交信息，确认是否提到待修复问题。
2. 查看 `TODO.md`、`PLAN.md`、工作区状态，确认第一个未完成任务及其上下文。
3. 评估该任务是否可在本轮完整闭环；若不可，则拆分并更新计划文件。
4. 阅读相关代码与测试，定位实现点和潜在既有问题。
5. 实施代码修改。
6. 运行相关测试与必要的质量检查（至少覆盖受影响范围；如可行，补充 `cargo fmt`、相关测试、`cargo clippy --all-targets -- -D warnings`）。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
8. 生成一次清晰的 Git 提交，仅覆盖本轮完成的逻辑闭环。

## 记录约定

- 若发现“最新提交提到的问题”，会在本文件新增“最新提交问题”小节并记录处理状态。
- 若发现任务需要拆分，会在本文件新增“任务拆分”小节，写明原因与新的执行顺序。
- 若发现阻塞性既有缺陷，会在本文件新增“阻塞问题”小节，写明规格缺口、影响面、已更新的任务顺序和停止原因。

## 当前进展（2026-04-25）

- 已检查最新提交 `e5a7aee [T5000b4cR] Review effect emitter context boundary`。提交说明本身未声明新的未修复缺陷；当前仍需继续按 `TODO.md` 执行下一条未完成任务。
- 已确认 `TODO.md` 的第一条未完成任务是 `T5000b4R Review：确认 MainCodegen 的上下文分层已经成立`，不需要再次拆分。
- 已完成本轮 review 的第一轮证据收集：
  - `CompilationUnitCodegenCx` 现集中持有编译单元级只读输入、共享 cache、`effect_op_tags` 与 `known_effect_instances_by_effect_fqn`；
  - `FunctionBodyCodegenCx` 现集中持有 env、loop、return、sret、top-level const eval stack 等函数/函数体生命周期状态；
  - `EffectLoweringCodegenCx` 现集中持有 effect return bridge、callee suspend/resume、continuation replay、suspend-site outcome capture 等 effect emitter 专属状态；
  - `state_machine_emitter.rs` 已通过 `take_*_cx` / `restore_*_cx` 与 `with_*` helper 成组切换 effect/runtime-function 语境，没有继续直接读写 `effect_cx` 内部字段。
- 当前未发现需要插入到 `T5000b4R` 之前的新前置缺陷任务。

## Review 观察

- 仍明显依附在 backend 主上下文上的“共享事实 / 中端分析”入口主要集中在：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中的 `HandlePlanContext::from_codegen(...)`；
  - 同文件中的 `ensure_known_fun_body_may_outward_effect_cache(...)` / `known_fun_body_may_outward_effect_map(...)`；
  - `crates/scoopc/src/llvm/codegen/mod.rs` 与 `state_machine_plan.rs` 各自维护的一套 concrete-type / field-type 恢复 helper。
- 这些泄漏点与 `T5000c` 已描述的 `ProgramFacts` / `EffectAnalysisCtx` / shared side tables 抽离方向一致；目前没有发现比 `T5000c` 更早、但尚未在 `TODO.md` 中跟踪的独立阻塞项。
- 下一步：运行本轮 review 所需验证命令；若通过，则回写 `TODO.md` / `PLAN.md` 并完成提交。

## 验证结果（2026-04-25）

- 已运行并通过：
  - `cargo test -p scoopc llvm::`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已将 `T5000b4R` 标记为完成，并把结论回写到 `TODO.md` / `PLAN.md`。
- 当前结论：
  - `MainCodegen` 的 module / function / cache / effect emitter 分层已经成立；
  - 下一条任务应切换到 `T5000bR Review`，重点扩大到整个 LLVM codegen 是否已经整体收口到 backend lowering；
  - 暂未发现需要插入在 `T5000bR` 之前的新前置缺陷任务。
