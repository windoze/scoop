# 本轮执行计划

## 约束说明

- 按用户要求，本文件先记录可公开的执行计划、决策依据摘要与后续进度。
- 不记录逐字内部推理；仅记录足以审计执行过程的步骤、依据和结论。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交说明里是否提到已知问题、临时修复或后续待修事项。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，确认该任务的上下文、依赖和拆分状态。
4. 若第一个未完成任务过大，则先把它拆成更小的子任务，并同步更新 `TODO.md` 与 `PLAN.md`；本轮仅执行拆分后的第一个子任务。
5. 若在检查、测试、实现过程中发现任何既有问题、规范不匹配、缺失特性、回归或不完整边界，则优先修复；若无法在本轮直接修复，则把它作为前置任务插入 `TODO.md` 当前任务之前，并更新 `PLAN.md` 后停止。
6. 对本轮目标任务进行实现。
7. 运行相关测试，并补充必要测试；同时运行格式化、静态检查和无告警检查。
8. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞关系。
9. 提交 Git commit，然后停止，不继续下一个任务。

## 当前状态

- 已创建执行记录文件。
- 已检查最新提交：`dba3a3b9 [T5000b4R] Review MainCodegen context layering`，提交说明未额外声明待修缺陷。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务是 `T5000bR Review：确认 LLVM codegen 已收口到“只做 backend lowering”的方向`。

## 当前公开结论摘要

- `MainCodegen` 的 module / function / cache / effect emitter 分层已经形成，`CompilationUnitCodegenCx`、`SharedCodegenCaches`、`FunctionBodyCodegenCx`、`EffectLoweringCodegenCx` 的结构边界可直接审计。
- 仍明显滞留在 LLVM backend 内、且下一步应迁出的 shared facts 包括：
  - `HandlePlanContext::from_codegen(...)` 直接从 `MainCodegen` 采集分析输入；
  - `known_fun_body_may_outward_effect_*` 缓存仍在 codegen 内构造 higher-order suspendability 事实；
  - `resolve_expr_concrete_type` 等 concrete-type / field-type 恢复逻辑在 backend 与 effect planning 侧重复出现；
  - `effect_step_summary.rs` 通过 `include!` 直接复用 `state_machine_plan.rs`，说明该分析已有 backend 外消费者。
- 在 review 取证过程中发现一个既有文档错配：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 顶部模块注释仍写“下一步 T5000b4”，但该任务已完成；
  - 这属于当前 review 范围内应顺手修正的注释问题。

## 接下来

1. 记录最终结果并提交 commit。

## 已完成事项

- 已修正 `crates/scoopc/src/llvm/codegen/mod.rs` 顶部注释中过期的“下一步 T5000b4”指向，改为准确说明下一步是 `T5000c` 的 shared-facts 抽离。
- 已将 `T5000bR` 的 review 结论回写到 `TODO.md` 与 `PLAN.md`，并把下一条待执行任务切换为 `T5000c`。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 结果：全部通过。

## 本轮结论

- 本轮任务 `T5000bR` 可判定为完成。
- 当前未发现需要插到 `T5000c` 之前的新前置缺陷任务。
- 本轮完成后应提交一次单独 commit 并停止，不进入 `T5000c`。
