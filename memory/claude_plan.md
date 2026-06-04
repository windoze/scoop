# 当前执行计划

## 原则
- 以 `TODO.md` 为唯一任务顺序和完成状态来源。
- 本次只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 不做开放式历史问题清扫；只处理当前任务直接需要或验证中暴露且未被计划覆盖的问题。
- 若发现当前任务被真实缺口阻塞，更新 `TODO.md` 添加最小前置任务并提交后停止。
- 不记录私有逐步推理；本文件记录可审查的执行计划、关键决策和进度。

## 步骤
1. 阅读 `TODO.md`，定位第一个未完成任务，并检查任务正文、依赖、验证要求和完成记录。
2. 检查最近提交是否明确提到与该任务直接相关的未完成事项；如有，将其纳入当前任务或作为前置任务写入 `TODO.md`。
3. 针对该任务读取必要代码、测试、文档和夹具，确认应修改的最小范围。
4. 实施任务要求；若遇到 spec 不匹配、缺失语言能力、运行时问题或不能接受的 workaround，优先修复同类根因，或在 `TODO.md` 中插入最小前置任务后停止。
5. 运行格式化与验证：先 `cargo fmt`，再 `cargo clippy --all-targets -- -D warnings`，然后根据任务要求运行相关测试、完整 Rust 测试和完整 fixture suite；如仅修改文档且已有可复用绿色结果，则在完成记录说明跳过理由。
6. 更新 `TODO.md`：将已完成任务标题加 `[DONE]`，填写完成记录、变更摘要和验证结果；仅当阶段计划实际变化时更新 `PLAN.md`。
7. 检查 git 状态和差异，提交本次所有相关变更，提交信息包含任务编号和简洁说明。
8. 提交后停止，不处理下一项任务。

## 进度记录
- 已建立本次执行计划，下一步读取 `TODO.md` 识别第一个未完成任务。
- 已定位第一个未完成任务：`T2-06`。任务目标是迁移 `physical_layout` / `global_init` / `class_ctor_inits` ownership，并删除 `LirArtifact.facts` 与 codegen 对 `LirFacts` 的消费。
- 最近提交为 `T2-05-R`，未发现与 `T2-06` 直接相关的额外未完成事项。
- 已实施第一轮迁移：`LateLoweredProgram` 新增 program/global/layout/type-context payload；class ctor init body 可挂载 init facts；`LirArtifact`、cached dep handoff、LLVM stage output 与 LLVM emit/codegen 生产路径不再携带独立 `LirFacts`。
- 已运行 `cargo fmt` 与 `cargo clippy --all-targets -- -D warnings`，clippy 通过。
- 验收搜索确认：`crates/scoopc_codegen_llvm/src` 中无 `LirFacts` 类型引用；`LirArtifact` 无 `facts` 字段；cone artifact manifest/持久化也不再写 `lir_facts.bin`。
- 后续又将 `LirStageOutput` 的独立 `LirFacts` 字段移除；`lir_facts()` 现在从 `LateLoweredProgram` 生成兼容快照。
- 重新运行完整验证全部通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。
- `TODO.md` 已将 `T2-06` 标记为 `[DONE]` 并写入完成记录。下一步检查 git 差异并提交本任务。
