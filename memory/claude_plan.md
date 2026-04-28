# 本轮执行计划

## 约束说明

出于信息安全与可维护性考虑，这里记录的是可审计的执行计划、判断依据摘要、关键决策与进度更新，不记录完整私有推理细节。

## 当前目标

按照 `TODO.md` 的顺序执行首个未完成任务；若在检查最新提交、代码、测试或实现边界时发现既有问题，则先修复该问题，或把它作为前置任务加入 `TODO.md` / `PLAN.md` 后停止。

## 步骤计划

1. 检查最新一次 Git 提交，确认是否明确提到尚未修复的问题、回归、临时方案或待补工作。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 阅读 `PLAN.md`，核对任务背景、依赖与当前实施状态。
4. 评估当前任务规模：
   - 如果任务足够小，直接实现。
   - 如果任务过大，先把它拆分为更小子任务，并更新 `TODO.md` 与 `PLAN.md`，本轮只执行新的首个子任务。
5. 在实现前补充必要上下文：定位相关模块、现有测试、规范或最近改动。
6. 实现任务，同时避免引入规避性修补；若暴露既有缺陷，优先修复或把缺陷登记为阻塞前置任务。
7. 运行与本任务直接相关的测试，再逐步扩大到必要的全量校验，至少包括：
   - 相关单元/集成/fixture 测试
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - 必要时执行更广的 `cargo test --all` 或项目约定命令
8. 更新文档与任务状态：
   - 在 `TODO.md` 标记完成，或在阻塞场景下重排任务依赖
   - 更新 `PLAN.md`
   - 持续更新本文件中的“进度记录”
9. 提交 Git commit，提交信息聚焦本轮完成事项，然后停止。

## 进度记录

- 已初始化本文件。
- 已检查最新 Git 提交 `920e287cabf3e3e31e6f58c641e56860a75d6644`，提交标题为 `[T5000j3a] Expand init raw MIR production coverage`；提交信息本身未直接声明一个尚未修复的既有问题。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务为 `T5000j3aR Review：确认 init 场景扩张只是放宽 canonical MIR 覆盖，而非把分析责任倒灌回 backend`。
- 当前复核重点：
  1. `emit` / `reachability` 是否仅消费既有 MIR materialization 与候选选择事实；
  2. `raw non-generic candidate` 放宽后，closure / fun-value / implicit tail return / ctor-call todo 等 unsupported shape 是否仍稳定 fallback；
  3. 是否存在需要先修复的新既有缺陷；若有，先修复或前插任务，再决定是否能完成 `T5000j3aR`。
- 复核中发现一个既有 reachability 缺口：
  - `crates/scoopc/src/llvm/reachability.rs` 虽已把 `object_inits` / `top_level_vars` 纳入 raw candidate 作用域判断，但实际扫描 `TopLevelRef` 时仍只递归 `top_level_consts` / `top_level_immutable_values`；
  - 这会导致“仅由 object init body 内部调用的 helper”可能只被声明、不被收进 reachable body 发射集合，属于必须先修的真实问题。
- 已实施修复：
  - 在 `reachability.rs` 中新增共享的顶层值引用扫描入口，并补上 `object init` / `top-level var` 的递归扫描与去重集合；
  - 已在 `crates/scoopc/src/llvm/tests.rs` 新增 production raw MIR 路径与 legacy HIR 路径的 object-init helper reachability 回归。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc object_init_helper_dependency -- --nocapture`
  - `cargo test -p scoopc production_codegen_ -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`（`fixtures: ok (1202)`）
- 已完成文档更新：
  - `TODO.md` 已将 `T5000j3aR` 标记为完成，并记录 review 发现/修复的 reachability 缺口与验证结果；
  - `PLAN.md` 已补记同样的 review 结论，并将下一条待执行任务推进到 `T5000j3b`。
- 当前剩余收尾：
  1. 检查工作区改动；
  2. 提交 Git commit；
  3. 停止，等待下一轮执行。
