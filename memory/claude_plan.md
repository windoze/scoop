## 本次执行计划

1. 读取 `TODO.md`，确认第一个未标记为 `[DONE]` 的任务，并检查最近一次提交是否包含与该任务直接相关且尚未完成的问题。
2. 阅读该任务涉及的代码、测试、规范说明与依赖记录，仅围绕当前任务建立上下文，不做开放式问题扫查。
3. 如任务可直接完成：按最小正确改动原则实现代码与测试，并在关键步骤完成后持续更新本文件。
4. 如遇到阻塞当前任务的真实缺陷或缺失能力：先确认其是否为当前任务的前置条件；若是，则更新 `TODO.md` 增加最小必要前置任务、说明依赖关系，并停止继续向后推进。
5. 运行任务要求的验证命令，以及必要的 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`（若适用于本次改动范围），修复发现的问题。
6. 完成后更新 `TODO.md`：将当前任务标题显式标记为 `[DONE]`，补全完成记录；仅在阶段计划发生变化时更新 `PLAN.md`。
7. 检查工作区变更，按仓库现有风格撰写一次原子提交，只完成当前这个任务，然后停止。

## 进度更新

- 已写入初始执行计划，下一步读取 `TODO.md` 确认当前任务。
- 已确认首个未完成任务为 `P8-T01`：删除顶层 legacy selector 与并行 dispatcher 壳层。
- 最近一次提交为 `P7-T04R` review，不包含当前任务的未完成前置修复说明；当前按 `P8-T01` 直接推进。
- 已定位本任务的主要改动面：
  - `crates/scoopc/src/session/mod.rs`：移除 `EffectPipelineMode` 与 session bifurcation。
  - `crates/scoopc/src/effect_refactor_pipeline/mod.rs` 及相关测试：删除 legacy/refactor dispatcher 分叉，收口为单一路径。
  - `crates/scoop/src/cli.rs`、`crates/scoopc/src/driver_cli.rs`、`crates/scoop/src/commands/mod.rs`：删除 CLI selector 解析与传递。
  - `crates/scoop/src/fixtures/**` 与 `crates/scoop/tests/p7_default_pipeline.rs`：删掉对 `--effect-pipeline` 与显式 legacy/refactor 模式的假设，改为验证“参数已移除 / 默认唯一主线”。
- 下一步：执行上述代码修改，并在修改完成后跑定向测试与 smoke 验证。
- 已完成的关键改动：
  - 删除 `crates/scoopc/src/session/mod.rs` 中的 `EffectPipelineMode` / selector 解析错误，`SessionOptions` 已收口为不承载 bifurcation 的空配置壳。
  - 删除 `scoop` / `scoopc` CLI 的 `--effect-pipeline` 解析与帮助文本；新增“参数已移除”的单元/集成负向断言。
  - 删除 `crates/scoopc/src/effect_refactor_pipeline/legacy.rs` / `refactor.rs` 与顶层 dispatcher 壳层；`effect_refactor_pipeline::mod` 现仅保留单一路径 stage API。
  - 删除 `crates/scoop/src/commands/parity.rs`，不再保留 compare/rollback 顶层测试胶水。
  - fixture/run-pass/build/frontend 路径已去掉 selector 注入与 session mode 分支，统一走唯一主线。
- 下一步：运行编译/测试，清理剩余尾部告警或失败，并补齐 `TODO.md` 完成记录所需的验证摘要。
- 已完成验证：
  - `cargo test -p scoop cli`
  - `cargo test -p scoopc session`
  - `cargo test -p scoopc driver_cli`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- dump-ast tests/fixtures/parse/hello.scoop`
  - `cargo run -p scoop -- build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p8_single_pipeline.ll`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`
  - `cargo run -p scoop -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`（预期失败，已确认参数不存在）
- 已更新 `TODO.md` 与 `TODO-P8.md`，将 `P8-T01` 显式标记为 `[DONE]` 并补充完成记录。
- 下一步：检查工作区变更，按 `P8-T01` 做一次原子提交后停止。
