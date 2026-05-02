# 执行计划

说明：按安全要求，这里记录可审计的执行计划、决策摘要与进度更新，不记录私有推理细节。

## 当前目标

完成 `TODO.md` 索引所指向的第一个未完成详细任务；若存在阻塞，则按要求补充最小前置任务并同步索引，随后提交并停止。

## 执行步骤

1. 读取 `TODO.md`，识别其引用的详细任务文件与任务顺序。
2. 按顺序读取相关 `TODO-Px.md`，找到第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交是否直接提到与该任务相关的未完成问题；若是，则将其视为该任务的一部分或必要前置。
4. 阅读与当前任务直接相关的代码、测试、规范和任务约束，确认实现边界与验证要求。
5. 实现任务；若遇到阻塞当前任务的真实缺口或规范不匹配，则先修复，或在对应 `TODO-Px.md` 中添加最小前置任务并同步 `TODO.md`。
6. 运行必要验证，包括与改动相关的测试；如适用，运行格式化、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`，或选择更小但足以证明当前任务正确性的验证集；若任务要求更强验证则遵从任务文件。
7. 更新 `memory/claude_plan.md` 记录关键进展与计划调整。
8. 在对应 `TODO-Px.md` 中将当前任务标题标记为 `[DONE]` 并补全完成记录；如任务元数据有变化，同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
9. 按仓库约定创建一次 git 提交，提交当前任务相关全部未提交改动，然后停止。

## 进度日志

- 已创建本计划文件，准备开始定位首个未完成详细任务。
- 已确认首个未完成详细任务为 `P6-T01R`（`TODO-P6.md`）。最近提交为 `[P6-T01] Route refactor build through explicit LLVM stage`，未单独记录额外未完成尾项。
- 已复跑核心验证：`cargo test -p scoopc refactor_llvm_codegen_stage`、legacy/refactor `build --emit-llvm`、refactor `build --emit-obj`、refactor `build --emit-asm`、refactor `run`；当前这些命令均通过。
- 已通过源码审阅确认 blocker：`crates/scoopc/src/llvm/emit.rs` 虽把 `LateLoweredProgram` 带入 refactor stage handoff，但当前只在 module 构建前检查入口 callable 是否存在；实际 lowering 仍经 `CompilationUnitCodegenCx` / `mir_body.rs` 走 legacy effect state-machine、handler-stack 与 `EffectSignal` / `EffectOutcome` 合同，因此当前无法完成 `P6-T01R` 对“已与 old effect backend 分离”的确认。
- 下一步：在 `TODO-P6.md` 中添加最小前置任务 `P6-T01a`，要求先禁止 refactor LLVM 路径在 effectful lowering 上静默回落到 legacy backend，并同步 `TODO.md`，随后提交并停止。
- 已完成任务拆分与索引同步：`TODO-P6.md` 新增 `P6-T01a`，`P6-T01R` 记录 blocker 并改为依赖 `P6-T01a`；`TODO.md` 已同步插入新索引项。`PLAN.md` 未改，因为阶段级顺序未变。
