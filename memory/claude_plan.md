# Autonomous execution plan

## Selected task

**P10-T06R：Review per-cone 并发 driver**（TODO-7.md:1269-1280）

P10-T06 已 [DONE]；按 TODO.md 顶部"执行顺序"说明，下一项是 P10-T06R（review per-cone 并发 driver）。

## Review checklist (摘自 TODO-7.md:1271-1278)

1. cone DAG 调度是否严格按依赖关系（不会让 consumer 在任一直接依赖 artifact 落盘前开跑）
2. `SubprocessConeCompiler` trait 是否真正抽象、driver 不直接 hardcode `Command::new("scoopc")`
3. 失败传播是否完整（子进程 stderr / exit code / diagnostic 都能在父进程聚合显示，并保留 cone 归属）
4. 默认 jobs 与 `--jobs 1` 行为是否都被测试覆盖；并发 vs 串行产物是否真的等价
5. `scoopc` 是否仍然没有承担 driver / 调度职责
6. cache hit 与子进程派发的混合场景是否仍正确注入下游 cone

## Validation steps (摘自 TODO-7.md:1279)

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

### Phase 1: 通读 P10-T06 实现，对照 review checklist 检查

1. `crates/scoop/src/commands/build/scheduler.rs` 全文：状态机/拓扑/并发/失败传播/单测覆盖
2. `crates/scoop/src/commands/build/concurrency.rs`：trait 定义 + LocalProcessConeCompiler 实现
3. `crates/scoop/src/commands/build.rs`：driver 接入位置
4. `crates/scoopc/src/driver_cli.rs` + `crates/scoopc/src/bin/scoopc.rs` + `crates/scoopc/src/single_cone.rs`：scoopc 子命令是否仅承担"编译单 cone"
5. cache-hit 路径：`crates/scoop/src/commands/build/incremental.rs` 与 scheduler 的 cache-hit 短路
6. fixture 验证：`source_path_dependency_public_call`

### Phase 2: 跑全部验证步骤

按 P10-T06 完成条件中列的 9 步全部执行。

### Phase 3: 决策

- 若 review 通过：mark P10-T06R 为 [DONE]，更新 TODO.md/TODO-7.md，commit
- 若发现阻塞性缺陷：直接修正（按 prompt"review 任务必须直接修正或阻塞下一任务"），commit，再 mark [DONE]
- 若发现非阻塞遗留：在 TODO 中记录但仍可 [DONE]

## Status
- [x] 读 TODO.md / TODO-7.md，确认 task 是 P10-T06R
- [x] Phase 1：通读 P10-T06 实现，6 项 review checklist 全部通过
- [x] Phase 2：跑验证（cargo fmt / clippy / build / test --all / fixture suite 1507/1507 / dependency-gate / git diff --check 全部 clean，手工时序复核 jobs=1 与 jobs=4 等价）
- [x] Phase 3：mark P10-T06R [DONE]，更新 TODO.md / TODO-7.md，commit
