# 执行计划

说明：按你的要求，我先把可公开的执行计划写入这里，再开始读取任务与执行命令。这里记录的是执行步骤、决策点、进度与变更，不包含内部私有推理细节。

## 初始计划

1. 读取 `TODO.md`，确认第一个标题未带 `[DONE]` 的任务，并以它作为本次唯一执行目标。
2. 检查最近提交是否直接提到与该任务相关且未完成的问题；如果该问题构成当前任务的直接前置阻塞，则先在 `TODO.md` 中补成最小前置任务并调整依赖/顺序。
3. 阅读该任务涉及的代码、测试、规范与相关文档，只收集完成当前任务所必需的信息，避免开放式问题扫荡。
4. 实现当前任务要求的代码改动；如遇到会破坏规范一致性的缺口、回归、实现边界或缺失特性，不绕过，转而修复它，或将其作为新的最小前置任务写入 `TODO.md`。
5. 运行与当前任务直接相关的验证，包括必要测试；如任务完成范围较大，再补充 `cargo test`、`cargo clippy --all-targets -- -D warnings` 等仓库要求的验证。
6. 更新文档记录：
   - 在 `TODO.md` 中将当前任务标题显式改为带 `[DONE]`，并补全完成记录；
   - 仅在阶段计划或依赖结构发生变化时更新 `PLAN.md`；
   - 在本文件持续记录关键进展、计划变化、阻塞与已完成步骤。
7. 按仓库约定创建一次 git 提交，提交信息以当前任务 ID 为前缀。
8. 完成后立即停止，不继续处理下一个任务。

## 进度记录

- 已完成：创建本计划文件。
- 已完成：读取 `TODO.md`，确认首个未完成任务是 `P7-T04`（运行 GC env 全开验证，并冻结 P7 -> P8 handoff）。
- 已完成：检查最近一次提交 `[P7-T03S] Close mixed replay verify-roots blocker`；未发现需要先补入 `TODO.md` 的新未完成前置项。
- 当前计划：
  1. 检查工作树状态，确认是否存在未提交改动需要一并吸收。
  2. 按 `P7-T04` 要求运行默认 refactor 下的 GC env 全开矩阵：`run-pass` 与 `runtime_gc`。
  3. 若矩阵失败，定位是否为当前任务直接暴露的真实 blocker；若可直接修复，则修复并重跑；若必须先引入新的最小前置任务，则更新 `TODO.md` 并停止。
  4. 若矩阵通过，补跑要求的显式 `legacy` smoke，并整理仍保留到 P8 的 legacy 入口/测试/文档摘要。
  5. 更新 `TODO-P7.md`、`TODO.md`（必要时）以及可能需要补充的 handoff 注释/文档；最后提交本任务并停止。
- 已完成：工作树检查确认本轮起始时仅有 `memory/claude_plan.md` 未提交。
- 已完成：默认 refactor GC env 全开矩阵通过：
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` → `fixtures: ok (391)`
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc` → `fixtures: ok (25)`
- 已完成：显式 `legacy` compare/rollback smoke 通过：
  - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/run-pass/minimal_main.scoop` → `fixtures: ok (1)`
  - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop` → `fixtures: ok (1)`
- 已完成：补强 `P7 -> P8` handoff 文档/注释，更新 `EFFECT_REFACTOR.md` 与 `crates/scoopc/src/effect_refactor_pipeline/mod.rs`。
- 已完成：把 `P7-T04` 标记为 `[DONE]`，并在 `TODO-P7.md` / `TODO.md` 中写回完成记录与索引状态。
- 已完成：运行 `cargo fmt --all`。
- 下一步：检查最终工作树并创建本轮 git 提交。
