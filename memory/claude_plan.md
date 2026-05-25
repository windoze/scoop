# Autonomous execution plan

## Selected task

**P10-T05R：Review CLI 参数与并发抽象**（TODO-7.md:1193-1203）

P10-T05 由上一轮提交（commit `60c72c61`）落地，下一轮按顺序应做 review 任务。

## Review focus (from TODO-7.md:1196-1201)

1. `--jobs` 默认值、环境变量与失败 diagnostic 是否合理；
2. `ConcurrencyStrategy` 与 `SubprocessConeCompiler` trait 是否真的做到"接口/抽象保留"，driver 调用点没有把固定策略 / 固定执行体硬编码；
3. 如果 `scoopc` 加了 "build-single-cone" 子命令，是否仍是单纯编译器（即没有 driver / scheduler / 多进程 fork 逻辑）；
4. default 行为是否完全保持 P10-T04 后的现状；
5. `PLAN.md` / `PIPELINE_REFACTOR.md` 的设计基线追加是否覆盖：cone DAG 并发约束、并发策略可订制、子进程 scoopc 抽象（为分布式编译预留）。

## Validation requirements (P10-T05R section + P10-T05 section reference)

重新运行 P10-T05 的所有验证：
1. `cargo fmt`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo build --workspace`
4. `cargo test --all --all-targets`（30min+ timeout）
5. `cargo run -p scoop -- build --help`（人工检查 `--jobs` 与 `-j` 出现）
6. `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`（行为不变；30min+）
7. `cargo run -p scoop_tools -- dependency-gate`
8. `git diff --check`

## Execution order

1. 通读 P10-T05 实际改动（`git show 60c72c61` + 相关源文件）
2. 验证 5 个 review focus：
   - cli.rs、commands/mod.rs、commands/build.rs、commands/build/concurrency.rs、commands/run.rs 是否符合设计
   - PLAN.md 与 PIPELINE_REFACTOR.md 的基线追加段落
   - dependency_gate（确认 scoop crate 不出现新的非法依赖方向）
3. 跑全量验证
4. 如发现缺陷或漏点：
   - 必须直接修正（review 不是形式检查）
   - 或者把缺陷转换为 prerequisite TODO 任务
5. 标 TODO.md / TODO-7.md 中 P10-T05R 为 [DONE]，提交

## Progress log
- 2026-05-25: 选定 P10-T05R；开始 review。
