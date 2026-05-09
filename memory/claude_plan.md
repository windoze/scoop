# 执行计划与进度日志

说明：按要求维护本文件，记录可共享的执行计划、关键决策、进度更新与阻塞项。不写入内部完整推理，仅保留简明的外部执行说明。

## 当前目标

完成 `TODO.md` 中按顺序排列的第一个未完成任务；若遇到阻塞，则在 `TODO.md` 中补充最小前置任务并停止。

## 初始执行计划

1. 读取 `TODO.md`，确认第一个未以 `[DONE]` 标记的任务。
2. 检查最近提交是否有与该任务直接相关且明确未完成的问题。
3. 阅读任务涉及的代码、测试、规范与相关文档，确认约束与验收方式。
4. 实现该任务要求的最小正确改动，不引入变通方案。
5. 运行任务要求的验证，以及相关测试、格式化、lint/编译检查。
6. 更新 `TODO.md` 的完成状态与完成记录；仅在阶段计划发生变化时更新 `PLAN.md`。
7. 提交本次改动，提交信息使用对应任务号。
8. 停止，不继续下一个任务。

## 进度更新

- 已创建本计划文件。
- 已读取 `TODO.md`，确认首个未完成任务为 `P8-T01R`（review 顶层 selector/dispatcher 删除结果）。
- 已检查最新提交 `[P8-T01] Remove top-level legacy pipeline selector`；提交信息未声明直接相关的未完成事项，继续按 `P8-T01R` 执行。

## 当前 review 执行步骤

1. 审查 `TODO-P8.md` 中 `P8-T01R` 的检查范围与验证要求。
2. 阅读 CLI、session、dispatcher 及相关测试/fixtures 代码，确认是否仍有可执行 legacy 顶层入口或隐藏切换点。
3. 运行要求的搜索与验证命令，复核 P8-T01 声称的删除结果。
4. 若发现阻塞 `P8-T01R` 的问题，优先修复；若无法当场修复，则在 `TODO.md` 中插入最小前置任务并停止。
5. 若 review 通过，则把 `P8-T01R` 标记为 `[DONE]`，补全完成记录并提交本次变更。

## Review 结果摘要

- 已审查 `crates/scoop/src/cli.rs`、`crates/scoop/src/commands/mod.rs`、`crates/scoopc/src/bin/scoopc.rs`、`crates/scoopc/src/session/mod.rs`、`crates/scoopc/src/effect_refactor_pipeline/mod.rs`、fixture helper 与 `crates/scoop/tests/p7_default_pipeline.rs`。
- `scoop` / `scoopc` CLI 均不再解析 `--effect-pipeline`；相关测试改为断言该参数被拒绝。
- `SessionOptions` 仍存在，但已是空配置壳，不再承载 legacy/refactor bifurcation；`Session::new()` / `with_options()` 只代表单一路径。
- `crates/scoopc/src/effect_refactor_pipeline/` 目录中已不存在 `legacy.rs` / `refactor.rs` dispatcher 子模块，只剩单一路径 stage 文件与顶层 API。
- 搜索 `--effect-pipeline|EffectPipelineMode|legacy.*selector|refactor.*selector` 后，代码命中只剩负向测试文案、单一路径注释，以及与 LLVM callable version 相关但无关 effect pipeline 的“selector”术语；未发现可执行 legacy 顶层入口或隐藏切换点。
- 定向验证已通过：
  - `cargo test -p scoop cli`
  - `cargo test -p scoopc session`
  - `cargo test -p scoopc driver_cli`
  - `cargo test -p scoop --test p7_default_pipeline`
  - `cargo run -p scoop -- dump-ast tests/fixtures/parse/hello.scoop`
  - `cargo run -p scoop -- build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p8_single_pipeline.ll`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`
  - `cargo run -p scoop -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`（按预期失败）
  - `cargo clippy --all-targets -- -D warnings`
