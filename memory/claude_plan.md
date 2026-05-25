# Autonomous execution plan

## Selected task

**P10-T06：在 scoop 中实现 per-cone 子进程并发编译 driver**（TODO-7.md:1213-1256）

P10-T05R 已完成；按 TODO.md 顶部"执行顺序"说明，下一项是 P10-T06（子进程 driver 实现），它同时会顺手收口 P10-T04-c（dep `LateLoweredProgram` / `.o` artifact handoff）。

## Goals (摘自 TODO-7.md)

- 在 `scoop` 编译 driver 中接入真正的 cone DAG 并发调度器：按拓扑顺序遍历 `SourceConeGraph::compilation_units()`，所有直接依赖都已落盘的 cone 推到 ready queue；并发度由 `ConcurrencyStrategy::max_concurrent_jobs` 控制
- 每个 cone 通过 `SubprocessConeCompiler::compile_cone(...)` 走子进程跑（默认 `LocalProcessConeCompiler`：fork+exec scoopc 的 single-cone 子命令）
- 子进程跑完按 P10-T02 已有 per-cone artifact 规范落盘；父进程仅校验 outputs.fingerprint 与 artifact 完整性后注入下游
- driver 与 trait 真正解耦——换一个 trait 实现就能替换执行体（fake compiler 测试证明）

## Required content (摘自 TODO-7.md)

1. cone DAG 调度器：状态机 Pending / Ready / InFlight / Done / Failed
2. 并发限制：`ConcurrencyStrategy::max_concurrent_jobs` 上限；超限排队；任何失败立即停止派发，已派发跑完汇总
3. 子进程执行：`std::process::Command` 跑 scoopc build-single-cone；stderr/stdout 由父进程聚合（保留 cone 标识）
4. artifact handoff：通过 CLI/文件/env 把上游 artifact 路径列表传给子进程；子进程通过磁盘 artifact 跟父进程通信；禁止通过 stdout 传 IR/AST/源码
5. cache hit：命中现有 artifact 时直接跳过子进程派发；mixed cache hit + dispatch 必须正确注入下游
6. `--jobs 1`：仍走调度器+子进程路径（fallback in-process 时必须显式记录覆盖测试）
7. 失败传播：精确定位失败 cone、保留 partial、退出码非零、聚合 diagnostic 带 cone 前缀
8. 测试：
   - 集成：(a) 单 cone；(b) 链式 dep->consumer；(c) dep_a / dep_b -> consumer 并发
   - 端到端 fixture：基于 source_path_dependency_public_call 跑 --jobs 4 vs --jobs 1
   - 失败传播：dep 失败导致 consumer 不启动且退出非零
   - trait 解耦：fake compiler 测试

## Validation steps

1. `cargo fmt`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo build --workspace`
4. `cargo test --all --all-targets`（≥30min timeout）
5. `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`
6. `cargo run -p scoop -- test`（完整 fixture suite，≥30min）
7. 手工时序复核：fixture `source_path_dependency_public_call` 在 `--jobs 1` vs `--jobs 4` 上 fresh build 与 cache hit 耗时
8. `cargo run -p scoop_tools -- dependency-gate`
9. `git diff --check`

## Execution plan

### Phase 1: 摸清现状
1. 读 scoop driver build 代码：commands/build.rs、commands/build/concurrency.rs、commands/build/incremental.rs
2. 读 scoopc 入口：bin/scoopc.rs、driver_cli.rs
3. 读 scoopc_cone artifact load/store API
4. 读 P10-T05 的 ConcurrencyStrategy / SubprocessConeCompiler trait
5. 读 P10-T03/T04 的 per-cone frontend 与 cache 路径
6. 理解 P10-T04-c 的阻塞点：consumer cache-hit 时 dep callable 的 `LateLoweredProgram` / `.o` 怎么 handoff

### Phase 2: 设计 single-cone 编译入口
1. scoopc 增加 `build-single-cone` 子命令（仅编译器职能：加载上游 artifacts → 跑本 cone 全 pipeline → 写 artifact）
2. CLI 形态：`--cone-id`、`--upstream-artifact <dir>...`、`--out <dir>`、`--profile`、`--inputs-fingerprint`
3. 子进程读上游 artifact，sustain frontend/mir/lir/codegen 全 pipeline 跑通
4. 子进程产出 frontend/effect_facts/mir/scoopir-export/lir_program/.o 全部 artifact
5. **同时收口 P10-T04-c**：dep 子进程必须产出非空 LIR 与 .o，consumer 子进程在 LLVM 阶段消费

### Phase 3: scoop driver 调度器
1. 实现 cone DAG scheduler（状态机：Pending/Ready/InFlight/Done/Failed）
2. 调用 `SubprocessConeCompiler::compile_cone`（默认 LocalProcessConeCompiler 走 fork+exec）
3. 集成 cache-hit fast path：对命中的 cone 跳过子进程
4. 失败传播：任何子进程失败立即停派，已派发跑完后聚合诊断
5. 父进程聚合子进程 stdout/stderr，带 cone 前缀

### Phase 4: 测试
1. unit/integration 测试：
   - cone DAG scheduler 状态机
   - 调度场景 (a/b/c)
   - RecordingFakeConeCompiler 验证 trait 解耦
   - 失败传播
2. fixture 测试：source_path_dependency_public_call
3. （可能新增多依赖 fixture）

### Phase 5: 文档与 PLAN
1. 更新 TODO.md / TODO-7.md（P10-T06 标 [DONE]，并视情况合并 P10-T04-c）
2. 视必要更新 PLAN.md / PIPELINE_REFACTOR.md（实质性边界变化时）
3. commit

## Progress log
- 2026-05-25: 选定 P10-T06；开始读代码摸清 P10-T05 trait surface 与 scoopc/cone artifact 现状。
- 2026-05-25: P10-T06 全部落地：
  - scoopc `build-single-cone` 子命令（`driver_cli.rs` + `bin/scoopc.rs` + `single_cone.rs`）；
  - scoop driver `commands/build/scheduler.rs` 状态机调度器（870 行 + 6 单测含 RecordingFake / BarrierFake 解耦证明）；
  - `LocalProcessConeCompiler` 实现真正 fork+exec scoopc 子进程派发（含 stderr 前缀 forward + 4 类结构化错误 + scoopc 二进制 fallback 定位）；
  - LLVM `RootCallableSelector::LibMode` + `emit_lib_obj_to_file_from_stage_output` 让 dep cone artifact 不要求 `fun main`；
  - frontend 配套：Lib consumer 可无 entry main 通过、cache-hit 改为"已写过不再重复写"语义、新增 `consumer_artifact_skeleton` skeleton 让 subprocess 装回非空 LIR/.o；
  - 验证全过：cargo fmt / clippy / build / test 全部通过、`cargo run -p scoop -- test` 1507/1507 fixture PASS、dependency-gate ok、git diff --check ok；
  - 时序复核（source_path_dependency_public_call）：jobs=1 fresh=41.4s/cache=8.3s，jobs=4 fresh=15.1s/cache=8.2s，两条 jobs 路径等价；
  - P10-T04-c 部分收口：dep cone 通过 subprocess 产出非空 LIR + .o，但 consumer cache-hit 真正零开销读 dep `lir_program.bin` 路径作为 follow-up 留给独立任务。
