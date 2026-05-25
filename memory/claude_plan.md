# Autonomous execution plan

## Selected task

按 TODO.md 索引的列出顺序，下一项 `[TODO]` 任务是 **P10-T04-c：让 cached dependency cone 在 LLVM codegen 层 callable_layouts/LateLoweredProgram 路径上可见**（TODO-7.md:1084）。

但状态注记明确提供了替代路径："下一项为 `P10-T04-c`（或先做 `P10-T05`，再用 `P10-T06` 顺手收 `P10-T04-c`）"。
P10-T04-c 的 spec 项 4 写"dep callable 的 LIR/codegen 必须由 dep 自己的子进程驱动"，这条要求需要 P10-T06 子进程基础设施。
P10-T04-c 自己的依赖注脚也写"建议与 P10-T06 / P10-T06R 协调收口"。

按 PROMPT 中"Missing or Incomplete Language Features"与"No Workarounds"原则，如果直接现在做 P10-T04-c，要么是 in-process 临时方案（违反 spec 项 4），要么必须把 P10-T05/T06 一并做。后者超出本轮单任务范围。

**结论：本轮先做 P10-T05（CLI 参数与并发抽象边界）。**
P10-T05 自身明确"本任务不引入任何并发执行行为"，是纯 CLI/trait/文档基线层任务，独立可完成。

需要在 TODO 索引层面把这一选择记录清楚：
- TODO.md 头部状态注记改为明确"已选择: 先做 P10-T05，P10-T04-c 推迟到 P10-T06 一并收口"。
- TODO-7.md 中：
  - P10-T04-c 的"依赖"行需要补"P10-T06 已先行实现"；
  - P10-T04R 的依赖保持（仍依赖 P10-T04-c），但执行顺序在 P10-T06 之后；
  - P10-T05 的"依赖"行从"P10-T04R"改为"P10-T04-b（前置，P10-T04R 留待 P10-T04-c 完成后做）"。
- TODO 索引行的状态保持不变，但执行顺序变为 P10-T05 → P10-T05R → P10-T06 → P10-T06R → P10-T04-c → P10-T04R → P10-T07 → P10-T07R。

## Bug recap (P10-T04-c, deferred)

- 复现：fixture `tests/fixtures/run_pass_cone/source_path_dependency_public_call`，cold → warm → `echo "" >> src/main.scoop` → 第三次 build。
- 错误：`crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/types.rs::callable_layout_by_root_fqn` 报 `LLVM ABI query 缺少 callable <dep.fqn> 的 published callable version`。
- 根因：cache-hit dep 时 dep 源已剔除，consumer 的 `LateLoweredProgram` 不包含 dep callable；现有 `lir_program.bin` 是空 `LateLoweredProgram`，`objs/` 目录为空。

## P10-T05 task scope

参考 TODO-7.md:1133-1186 的完整 spec。简要总结：

### CLI surface
- `crates/scoop/src/cli.rs`：`Build` 与 `Run` 子命令加 `-j / --jobs N` 参数（type=`usize`）；非数字 / 0 / 负值给 diagnostic。
- `SCOOP_BUILD_JOBS` env var fallback。
- 默认值用代码内常量 `DEFAULT_BUILD_JOBS = 4`。

### Concurrency abstraction
- `crates/scoop/src/commands/build/concurrency.rs`（新模块）。
- `pub trait ConcurrencyStrategy { fn max_concurrent_jobs(&self) -> usize; }`
- `pub struct FixedJobsStrategy { jobs: usize }` impl trait，trait 文档明确"未来按 CPU 数 / 内存 / 远端 worker 池策略可在此挂接"。

### Subprocess compiler abstraction
- `pub trait SubprocessConeCompiler`：定义跑单个 cone 的最小接口。
- 输入：cone 标识 / 上游已落盘 artifact 集合 / 本 cone 输入 fingerprint / artifact 输出目录。
- 输出：本 cone artifact 路径与 outputs.fingerprint。
- `pub struct LocalProcessConeCompiler`：占位实现，本任务允许 `compile_cone` 返回 `unimplemented!()` 或 stable error，trait + 占位类型必须就位。

### scoopc subcommand (optional)
- 加 `scoopc build-single-cone` 子命令，仅作 CLI surface（不引入调度/多进程）。
- 实现限定在"加载上游 artifacts → 跑 P10-T03 的 per-cone frontend orchestration → 写 artifact"。

### Documentation
- `PLAN.md` §4/P10 加"per-cone 多进程并发编译 CLI + driver 抽象"条目。
- `PIPELINE_REFACTOR.md` 编译顺序模型一节追加"cone DAG 中互不依赖的 cone 应允许并发跑、scoop 拥有 driver / scoopc 仅做 single-cone 编译执行体"约束。

### Tests
- CLI parse: `-j N`、`--jobs N`、默认值、invalid value（非数字 / 0 / 负值）。
- trait: fixed strategy `max_concurrent_jobs` 返回值。
- `LocalProcessConeCompiler` 占位 surface（"未实现行为"的稳定错误形态）。
- 如加 scoopc 子命令：happy path 解析 + 缺 artifact 错误。

### Validation
- `cargo fmt`
- `cargo clippy --all-targets -- -D warnings`
- `cargo build --workspace`
- `cargo test --all --all-targets` (timeout 30min+)
- `cargo run -p scoop -- build --help`（人工检查 `--jobs` 与 `-j` 出现）
- `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`（行为不变；30min+）
- `cargo run -p scoop_tools -- dependency-gate`
- `git diff --check`

### Completion criteria
- CLI / trait surface 落地，default 行为与 P10-T04 后等价；
- driver 调用点已经能注入 `ConcurrencyStrategy` 与 `SubprocessConeCompiler`，本任务不要求实际并发；
- `scoopc` binary 没有引入任何"驱动/调度"职责；
- `PLAN.md` / `PIPELINE_REFACTOR.md` 已经把 per-cone 多进程并发编译纳入设计基线。

## Execution order

1. 探索现状代码（`crates/scoop/src/cli.rs`、`commands/build.rs`、`scoopc::driver_cli`、`scoopc::frontend::run_frontend_with_artifact_cache`）。
2. 在 TODO.md / TODO-7.md 头部记录顺序变更。
3. 加 CLI 参数 `-j/--jobs`，环境变量 fallback，默认常量。
4. 新建 `concurrency.rs` 模块，定义两个 trait 和默认实现。
5. 把 trait 注入 `BuildOptions` 主流程（保持单进程顺序运行行为）。
6. 加 `scoopc build-single-cone` 子命令（CLI parse only，调用现有 per-cone API）。
7. 更新 PLAN.md / PIPELINE_REFACTOR.md。
8. 写单测覆盖 CLI / trait / placeholder。
9. 跑全套验证。
10. 更新 TODO.md / TODO-7.md，标 P10-T05 [DONE]，commit。

## Progress log
- 2026-05-25: 选定 P10-T05；确认 P10-T04-c 推迟到 P10-T06 之后处理。
