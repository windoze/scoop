# Test Infrastructure Cleanup TODO 索引

> 生成时间：2026-05-26
> 计划基线：[`PLAN.md`](./PLAN.md)
> 设计基线：[`TEST_INFRA_CLEANUP.md`](./TEST_INFRA_CLEANUP.md)
> 归档前置基线：
> - [`docs/archive/designs/PIPELINE_REFACTOR.md`](./docs/archive/designs/PIPELINE_REFACTOR.md)
> - [`docs/archive/designs/PIPELINE-CLEANUP.md`](./docs/archive/designs/PIPELINE-CLEANUP.md)
> - [`docs/archive/plans/PLAN-pipeline-refactor.md`](./docs/archive/plans/PLAN-pipeline-refactor.md)
> - [`docs/archive/plans/TODO-pipeline-refactor.md`](./docs/archive/plans/TODO-pipeline-refactor.md)
> 当前状态：P0-P4 全部 [TODO]；上一轮 Pipeline Refactor（P0-P10）已全部完成并归档。

## 总原则

- `PLAN.md` 是当前执行计划基线；如果实现时发现阶段边界、命令面契约或 fixture 发现规则需要改变，必须先回写 `TEST_INFRA_CLEANUP.md`，再调整 TODO。
- 所有任务按 `P0 → P1 → P2 → P3 → P4` 顺序推进；P3 严格在 P1+P2 完成之后执行，否则 CI 与开发流会断；P2 内部条目可与 P1 并行。
- 每个实现小阶段后必须紧跟一个独立 review 任务，复审该小阶段的完整变更、阶段目标和约束遵守情况。
- review 任务不是形式检查；如果发现前一任务没有真正完成目标，review 任务必须直接修正或阻塞下一任务。
- 任务完成后必须同时更新 `TODO.md` 中的任务状态与完成记录。
- 所有外部脚本统一用 **python（仅标准库）** 实现，放在 `tools/` 下；不复活 `tools/scoop_tools/`，不新增任何 Rust 工具 crate。
- shell 脚本（`tools/*.sh`）仅用于轻量编排，不实现指令解析 / golden 比对。
- `scoop` / `scoopc` 不得保留任何 `fixture` 命名的模块、CLI、env、API；`tests/fixtures/**` 对编译器而言只是普通源文件。
- `docs/archive/**` 整体冻结；`TODO*-pipeline-refactor.md` / `PLAN-pipeline-refactor.md` 等已归档文件中的 “验证通过：…” 行属于历史审计记录，不动。

## 任务索引

| 任务 | 状态 | 目标 |
| --- | --- | --- |
| [DONE] P0-T01 | [DONE] | 盘点 `EXPECT-*` 指令清单（语法/语义/参数）并落地 `docs/fixtures.md` 或 `tools/README.md` 一节 |
| [DONE] P0-T01R | [DONE] | Review `EXPECT-*` 指令清单完整度（覆盖现有 expectations.rs 全部指令） |
| [DONE] P0-T02 | [DONE] | 盘点 fixture 发现规则（phase router / `plan_targets` / `is_run_pass_cone_case_root` 等子目录约定） |
| P0-T02R | [TODO] | Review fixture 发现规则盘点结果（python 平迁可读性 + 现有 fixture 全覆盖） |
| P0-T03 | [TODO] | 冻结编译器对外命令的 stdout/stderr/exit-code 契约（`scoopc dump-*` / `emit-artifact` / `build-single-cone` / `link-cone` / `scoop build` / `scoop run`） |
| P0-T03R | [TODO] | Review 命令面契约冻结结果（外部 runner 需消费的字段已稳定） |
| P0-T04 | [TODO] | 删除 fixture-runner 自检 fixture（依赖 `EXPECT-ERROR-CODE: scoop::fixtures::*` 的 4 条：`timeout_should_fail.scoop` / `exit_code_mismatch.scoop` / `stderr_mismatch_distinguishable.scoop` / `gc_runner_stdout_mismatch_diagnostic_is_stable.scoop`）；同步复核 scoopc 内部确无 cfg(test) 旁路或文件名特判 |
| P0-T04R | [TODO] | Review fixture-runner 自检 fixture 删除结果（`grep -rn "scoop::fixtures::" tests/fixtures/ --include="*.scoop"` 无命中；旧 runner 在剩余 fixture 集合上仍跑通；checks 计数下降量与删除条数一致） |
| P1-T01 | [TODO] | `tools/run_fixtures.py`：fixture runner（phase 发现 + `EXPECT-*` 解析 + 子进程驱动 + golden 比对 + 多进程调度 + 超时/SIGKILL） |
| P1-T01R | [TODO] | Review `run_fixtures.py` 与旧 `scoop test` 在 `tests/fixtures/**` 上 pass/fail 集合与 checks 计数等价性 |
| P1-T02 | [TODO] | `tools/spec_fixtures.py {sync,check}`：替代 `scoop_tools spec-fixtures` |
| P1-T02R | [TODO] | Review `spec_fixtures.py` 与旧实现语义等价性 |
| P1-T03 | [TODO] | `tools/fixtures_matrix.py {check,stdlib}`：替代 `scoop_tools fixtures-matrix` |
| P1-T03R | [TODO] | Review `fixtures_matrix.py` 与旧实现语义等价性 |
| P1-T04 | [TODO] | `tools/safepoint_baseline.py`：替代 `scoop_tools safepoint-baseline` |
| P1-T04R | [TODO] | Review `safepoint_baseline.py` 与旧实现语义等价性 |
| P1-T05 | [TODO] | `tools/dependency_gate.py`：替代 `scoop_tools dependency-gate`（建议 `cargo metadata --format-version 1` JSON 驱动） |
| P1-T05R | [TODO] | Review `dependency_gate.py` 在当前仓库状态下与旧 Rust 版本结论一致 |
| P1-T06 | [TODO] | `tools/audit_spec_coverage.py`：从 `crates/scoopc/src/audit/spec_coverage.rs` 平迁 |
| P1-T06R | [TODO] | Review `audit_spec_coverage.py` 与旧 `#[cfg(test)]` 模块输出等价性 |
| P1-T07 | [TODO] | `tools/audit_pipeline_gap.py`：从 `crates/scoopc/src/pipeline_gap_audit.rs` 平迁 |
| P1-T07R | [TODO] | Review `audit_pipeline_gap.py` 与旧 `#[cfg(test)]` 模块输出等价性 |
| P1-T08 | [TODO] | `tools/audit_user_visible_failure_policy.py`：从 `crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 平迁 |
| P1-T08R | [TODO] | Review `audit_user_visible_failure_policy.py` 与旧 `#[cfg(test)]` 模块输出等价性 |
| P2-T01 | [TODO] | `.github/workflows/ci.yml` 切换（`scoop_tools` 调用 → `python3 tools/spec_fixtures.py check`；`cargo run -p scoop -- test` → `python3 tools/run_fixtures.py`） |
| P2-T01R | [TODO] | Review CI 切换结果（CI 跑通 + 不再出现旧入口） |
| P2-T02 | [TODO] | `tools/run_fixture_scan.sh` / `tools/run_run_pass_gc_scan.sh` / `tools/gc_microbench.sh` 内部调用串切换 |
| P2-T02R | [TODO] | Review shell 脚本切换结果（端到端跑通） |
| P2-T03 | [TODO] | `AGENTS.md` 更新（[`TEST_INFRA_CLEANUP.md` §6](./TEST_INFRA_CLEANUP.md#6-文档更新) 列点） |
| P2-T03R | [TODO] | Review `AGENTS.md` 更新（无旧入口残留） |
| P2-T04 | [TODO] | `README.md` 更新（fixture runner 描述 + 命令示例） |
| P2-T04R | [TODO] | Review `README.md` 更新（无旧入口残留） |
| P2-T05 | [TODO] | `tools/README.md` 整体重写为 python 脚本列表 |
| P2-T05R | [TODO] | Review `tools/README.md` 重写结果（覆盖所有 P1 脚本） |
| P2-T06 | [TODO] | `PROMPT.md` 更新（fixture-suite 命令与超时说明） |
| P2-T06R | [TODO] | Review `PROMPT.md` 更新（无旧入口残留） |
| P2-T07 | [TODO] | `SCOOP_FULL_SPEC.md` 更新（spec 内 `cargo run -p scoop -- test` / `scoop test` 调用串） |
| P2-T07R | [TODO] | Review `SCOOP_FULL_SPEC.md` 更新（无旧入口残留） |
| P2-T08 | [TODO] | `tests/fixtures/umb_fix/B-15-when-pattern/_README.md` 与其他 `tests/fixtures/**/_README.md` 调用串替换 |
| P2-T08R | [TODO] | Review fixture README 替换结果（grep 无旧入口） |
| P2-T09 | [TODO] | `docs/safepoint_baseline.md` 内 `safepoint-baseline` 调用切换 |
| P2-T09R | [TODO] | Review `docs/safepoint_baseline.md` 切换结果 |
| P3-T01 | [TODO] | 删除 `crates/scoopc/src/fixtures/{mod,expectations,run_pass}.rs` 与 `crates/scoopc/src/fixture_cli.rs` |
| P3-T01R | [TODO] | Review fixture runner 引擎删除结果（编译器内部不再持有 fixture 概念） |
| P3-T02 | [TODO] | 删除 `scoopc` `driver_cli.rs` 中 `CompilerCli::TestFixtures` 变体 / 路由 / `parse_test_fixtures` / USAGE；删除 `bin/scoopc.rs` dispatch；删除 `lib.rs` 模块导出 |
| P3-T02R | [TODO] | Review `scoopc` CLI 表面删除结果（`scoopc test-fixtures` 报未知子命令） |
| P3-T03 | [TODO] | 删除 `scoop` `cli.rs` `Command::Test { .. }` 变体与 `test_command_parses_*` 单测；删除 `commands/test.rs`；删除 `commands/mod.rs` dispatch 分支 |
| P3-T03R | [TODO] | Review `scoop test` 子命令删除结果（`scoop test` 报未知子命令） |
| P3-T04 | [TODO] | 删除 `crates/scoopc/src/audit/{mod,spec_coverage}.rs` / `pipeline_gap_audit.rs` / `pipeline_user_visible_failure_policy.rs`；删除 `lib.rs` `#[cfg(test)] mod` 挂载点 |
| P3-T04R | [TODO] | Review 审计模块删除结果（`scoopc` 不再 grep 文件） |
| P3-T05 | [TODO] | 删除 `tools/scoop_tools/` 整个 crate；从 workspace 根 `Cargo.toml` 删除 `"tools/scoop_tools",` 成员条目 |
| P3-T05R | [TODO] | Review `tools/scoop_tools/` 删除结果（`cargo metadata` 不再列出该 crate） |
| P3-T06 | [TODO] | 清理 `crates/scoop/tests/p8_docs_cleanup.rs` 对 `tools/scoop_tools/src/fixtures_matrix.rs` 的源路径引用 |
| P3-T06R | [TODO] | Review `p8_docs_cleanup.rs` 清理结果（按测试实际职责整体删除或更新路径） |
| P3-T07 | [TODO] | 清理 P3-T01/P3-T04 删除后产生的死代码、孤立常量、`SCOOP_FIXTURE_*` env 名字常量等 |
| P3-T07R | [TODO] | Review P3 残留清理结果（`crate::fixtures::` / `crate::fixture_cli::` / `SCOOP_FIXTURE_*` 全仓 grep 在源码内无命中） |
| P4-T01 | [TODO] | 全仓 grep 验证（[`TEST_INFRA_CLEANUP.md` §7 步骤 5](./TEST_INFRA_CLEANUP.md#7-实施顺序建议) token 清单），白名单：`docs/archive/**` 与 `TODO*-pipeline-refactor.md` / `PLAN-pipeline-refactor.md` 历史 “验证通过：…” 行 |
| P4-T02 | [TODO] | `cargo metadata --format-version 1 \| grep scoop_tools` 无命中验证 |
| P4-T03 | [TODO] | `python3 tools/run_fixtures.py` 与旧 `scoop test` 在最近基线上 pass/fail 集合 + checks 计数一致；如有 diff 必须定位修复 |
| P4-T04 | [TODO] | `python3 tools/{spec_fixtures,fixtures_matrix,safepoint_baseline,dependency_gate,audit_*}.py` 全部跑通；`dependency-gate` 与旧 Rust 版本结论一致 |
| P4-T05 | [TODO] | CI 在切换后跑通；本地 `cargo fmt` / `cargo clippy --all-targets -- -D warnings` / `cargo test --all --all-targets` 通过 |
| P4-T06 | [TODO] | `tools/run_fixture_scan.sh` / `tools/run_run_pass_gc_scan.sh` 用新入口跑通 |
| P4-T07R | [TODO] | Review P4 全包完成度（[`TEST_INFRA_CLEANUP.md` §8](./TEST_INFRA_CLEANUP.md#8-验证清单) 验收清单逐项核对） |

## 完成记录

> 任务完成后在此追加 `[DONE] PX-TNN：…（YYYY-MM-DD）` 行，附核心验证命令与产出。

- [DONE] P0-T01：新增 `docs/fixtures.md`，从 `crates/scoopc/src/fixtures/expectations.rs` 盘点当前 22 个 directive 前缀，并记录语法、参数、解析边界与各 phase 语义。验证：`python3` parser/doc 覆盖检查通过（22/22）；`cargo fmt --check` 通过。（2026-05-26）
- [DONE] P0-T01R：复审 `docs/fixtures.md` 与 `crates/scoopc/src/fixtures/expectations.rs`，确认 22 个 `strip_prefix` directive 前缀、`EXPECT: pass|ok|fail` 语义、header 扫描规则、共享输入与 parse/build/run-pass 消费语义均已覆盖，无需补充文档。验证：自定义 `python3` 前缀覆盖检查（22/22）；`cargo fmt --check`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`。（2026-05-26）
- [DONE] P0-T02：扩展 `docs/fixtures.md`，盘点 `plan_targets` 目标规划、phase router、`is_run_pass_cone_case_root`、multi/cone case 子目录约定、sysroot overlay 发现与 `umb_fix` 子路由，并给出可供 python runner 平迁的伪代码。验证：自定义 `python3` 文档 token 覆盖检查；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`。（2026-05-26）
