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
| [DONE] P0-T02R | [DONE] | Review fixture 发现规则盘点结果（python 平迁可读性 + 现有 fixture 全覆盖） |
| [DONE] P0-T03 | [DONE] | 冻结编译器对外命令的 stdout/stderr/exit-code 契约（`scoopc dump-*` / `emit-artifact` / `build-single-cone` / `link-cone` / `scoop build` / `scoop run`） |
| [DONE] P0-T03R | [DONE] | Review 命令面契约冻结结果（外部 runner 需消费的字段已稳定） |
| [DONE] P0-T04 | [DONE] | 删除 fixture-runner 自检 fixture（依赖 `EXPECT-ERROR-CODE: scoop::fixtures::*` 的 4 条：`timeout_should_fail.scoop` / `exit_code_mismatch.scoop` / `stderr_mismatch_distinguishable.scoop` / `gc_runner_stdout_mismatch_diagnostic_is_stable.scoop`）；同步复核 scoopc 内部确无 cfg(test) 旁路或文件名特判 |
| [DONE] P0-T04R | [DONE] | Review fixture-runner 自检 fixture 删除结果（`grep -rn "scoop::fixtures::" tests/fixtures/ --include="*.scoop"` 无命中；旧 runner 在剩余 fixture 集合上仍跑通；checks 计数下降量与删除条数一致） |
| [DONE] P1-T00 | [DONE] | 新增通用 `scoopc check-source` 命令面（非 fixture API）：支持 `parse` / `resolve` / `typecheck` / `infer` 的 phase-only 校验，支持单文件与 cone project 输入、`--source <path>` 选择项目内源文件、`--target-platform <id>` 覆盖；stdout/stderr/exit-code 契约写入 `docs/fixtures.md`，供外部 runner 避免用 `dump-*` / `build-*` 工作绕过 typecheck-only 语义 |
| [DONE] P1-T00R | [DONE] | Review `scoopc check-source` 命令面：确认它不引入 fixture 概念、不使用 `fixture` 命名 API/env，且能覆盖当前 `resolve` / `typecheck` / `infer` 单文件、多文件、cone case 的 phase-only 诊断需求 |
| [DONE] P1-T01 | [DONE] | `tools/run_fixtures.py`：fixture runner（依赖 P1-T00R；phase 发现 + `EXPECT-*` 解析 + 子进程驱动 + golden 比对 + 多进程调度 + 超时/SIGKILL） |
| [DONE] P1-T01R | [DONE] | Review `run_fixtures.py` 与旧 `scoop test` 在 `tests/fixtures/**` 上 pass/fail 集合与 checks 计数等价性 |
| [DONE] P1-T02 | [DONE] | `tools/spec_fixtures.py {sync,check}`：替代 `scoop_tools spec-fixtures` |
| [DONE] P1-T02R | [DONE] | Review `spec_fixtures.py` 与旧实现语义等价性 |
| [DONE] P1-T03 | [DONE] | `tools/fixtures_matrix.py {check,stdlib}`：替代 `scoop_tools fixtures-matrix` |
| [DONE] P1-T03R | [DONE] | Review `fixtures_matrix.py` 与旧实现语义等价性 |
| [DONE] P1-T04 | [DONE] | `tools/safepoint_baseline.py`：替代 `scoop_tools safepoint-baseline` |
| [DONE] P1-T04R | [DONE] | Review `safepoint_baseline.py` 与旧实现语义等价性 |
| [DONE] P1-T05 | [DONE] | `tools/dependency_gate.py`：替代 `scoop_tools dependency-gate`（建议 `cargo metadata --format-version 1` JSON 驱动） |
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

## 调整记录

- 2026-05-26：执行 P1-T01 前验证发现现有公开命令面无法 spec-correct 地平迁 resolve/typecheck-only fixture 语义：`dump-hir` 会漏掉旧 typecheck runner 覆盖的诊断，`build-single-cone` 又会运行到 HIR/lowering/codegen 并误伤通过的 typecheck fixture。因此在 P1-T01 前新增 P1-T00/P1-T00R，先落地通用非 fixture 的 `scoopc check-source` phase-only 校验入口；P1-T01 显式依赖 P1-T00R。

## 完成记录

> 任务完成后在此追加 `[DONE] PX-TNN：…（YYYY-MM-DD）` 行，附核心验证命令与产出。

- [DONE] P0-T01：新增 `docs/fixtures.md`，从 `crates/scoopc/src/fixtures/expectations.rs` 盘点当前 22 个 directive 前缀，并记录语法、参数、解析边界与各 phase 语义。验证：`python3` parser/doc 覆盖检查通过（22/22）；`cargo fmt --check` 通过。（2026-05-26）
- [DONE] P0-T01R：复审 `docs/fixtures.md` 与 `crates/scoopc/src/fixtures/expectations.rs`，确认 22 个 `strip_prefix` directive 前缀、`EXPECT: pass|ok|fail` 语义、header 扫描规则、共享输入与 parse/build/run-pass 消费语义均已覆盖，无需补充文档。验证：自定义 `python3` 前缀覆盖检查（22/22）；`cargo fmt --check`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`。（2026-05-26）
- [DONE] P0-T02：扩展 `docs/fixtures.md`，盘点 `plan_targets` 目标规划、phase router、`is_run_pass_cone_case_root`、multi/cone case 子目录约定、sysroot overlay 发现与 `umb_fix` 子路由，并给出可供 python runner 平迁的伪代码。验证：自定义 `python3` 文档 token 覆盖检查；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`。（2026-05-26）
- [DONE] P0-T02R：复审 `docs/fixtures.md` 与 `crates/scoopc/src/fixtures/mod.rs`，确认 target planning、phase routing、case-root predicates、sysroot overlay 跳过、`umb_fix` 子路由与现有 fixture 目录均已覆盖；补充记录 manifest-only `tests/fixtures/cone/` 不产生 fixture target。验证：自定义 `python3` fixture discovery 覆盖检查（1455 ordinary targets；resolve/typecheck/run_pass cone/multi cases 全覆盖）；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`（1536 checks）。（2026-05-26）
- [DONE] P0-T03：在 `docs/fixtures.md` 冻结外部 runner 允许调用的 `scoopc dump-*` / `dump-rtti` / `dump-stackmaps` / `emit-artifact` / `build-single-cone` / `link-cone` 与 `scoop build` / `scoop run` stdout、stderr、exit-code、数据产物契约；新增 `p8_docs_cleanup` 文档守卫测试防止命令面契约遗漏。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`（1536 checks）。（2026-05-26）
- [DONE] P0-T03R：复审 `docs/fixtures.md` 与实际 `scoopc` tool CLI / `scoop` facade 命令面，补强 `scoop run` 的程序退出码、stdin 继承与诊断流边界，并把 `scoop build` / `scoop run` 的 `--entry-package` 稳定参数纳入契约守卫。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`（1536 checks）。（2026-05-26）
- [DONE] P0-T04：删除 4 条依赖 `EXPECT-ERROR-CODE: scoop::fixtures::*` 的 fixture-runner 自检 `.scoop` fixture，并同步删除仅供这些 fixture 使用的 golden stdout/stderr 文件；复核 `tests/fixtures/**/*.scoop` 已无 `scoop::fixtures::` 期望，`crates/scoopc/src/**/*.rs` 已无 `timeout_should_fail` / `TIMEOUT_SHOULD_FAIL` 特判命中。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`（1532 checks，较 1536 基线减少 4）；`grep -rn --include='*.scoop' 'scoop::fixtures::' tests/fixtures/` 无命中；`grep -Ern --include='*.rs' 'timeout_should_fail|TIMEOUT_SHOULD_FAIL' crates/scoopc/src` 无命中。（2026-05-26）
- [DONE] P0-T04R：复审 P0-T04 删除结果，确认最新提交删除 4 条 fixture-runner 自检 `.scoop` fixture 及其专用 golden 文件，`tests/fixtures/**/*.scoop` 无 `scoop::fixtures::` 期望残留，旧 `scoop test` runner 在剩余集合上通过且 checks 计数为 1532，符合较 1536 基线减少 4 条的预期。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`（1532 checks）；`rg 'scoop::fixtures::' tests/fixtures --glob '*.scoop'` 无命中。（2026-05-26）
- [DONE] P1-T00：新增非 fixture 的 `scoopc check-source` 命令面，支持 `parse` / `resolve` / `typecheck` / `infer` phase-only 校验、单文件输入、cone project 输入、`--source <path>` 选择项目 source graph 内源文件、`--target-platform <id>` 平台覆盖，并把 stdout/stderr/exit-code 契约写入 `docs/fixtures.md`。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`target/debug/scoopc check-source` 单文件/target-platform/cone-project smoke；`cargo test --all --all-targets`；`cargo run -p scoop -- test`（1532 checks）。（2026-05-26）
- [DONE] P1-T00R：复审 `scoopc check-source` 命令面，确认新增入口本身不持有 fixture 概念且未使用 `SCOOP_FIXTURE_*` 等 fixture API/env；修复 `--source` 选择项目内单个 typecheck 源文件时被未选中失败 sibling body 诊断阻塞的问题，使多文件与 cone case 可按源文件逐项比对 pass/fail。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；新增 `tool_commands::tests::check_source_typecheck_source_selection_does_not_check_unselected_bodies`；`target/debug/scoopc check-source` mixed typecheck_multi/typecheck_cone smoke；`cargo test --all --all-targets`；`cargo run -p scoop -- test`（1532 checks）。（2026-05-26）
- [DONE] P1-T01：新增 `tools/run_fixtures.py`，以 python 标准库平迁 fixture target planning、`EXPECT-*` 解析、golden 比对、子进程驱动、多进程调度与 run-pass timeout/SIGKILL；同步补强 `scoopc check-source` 诊断定位输出、sysroot overlay typecheck 与 `dump-ir` materialized MIR 输出，使外部 runner 可覆盖现有 1532 checks。验证：`cargo fmt --check`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py tests/fixtures`（1532 checks）；`cargo run -p scoop -- test`（1532 checks）。（2026-05-26）
- [DONE] P1-T01R：复审 `tools/run_fixtures.py` 与旧 `scoop test` / `scoopc test-fixtures` runner，确认 target planning、phase routing、`EXPECT-*` directive 解析、run-pass golden/timeout/env/exit handling、summary/check 计数均保持等价；`tests/fixtures/**` 上新旧 runner 均为 1503 PASS targets、0 failures、1532 checks，排序后的 target/status/check 列表完全一致。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py tests/fixtures`（1532 checks）；`cargo run -p scoop -- test`（1532 checks）；自定义日志解析比对确认 target/status/check 列表一致。（2026-05-26）
- [DONE] P1-T02：新增 `tools/spec_fixtures.py`，以 python 标准库平迁 `spec-fixtures sync` / `check` / `check --fix` 的 fenced code block 抽取、`// FIXTURE:` 路径校验、重复路径检测、`tests/fixtures/spec_doctest/` 写入/比对与 stale `.scoop` 清理语义；当前 spec doctest 生成集合与旧 `scoop_tools spec-fixtures` 保持一致（1 个 fixture）。验证：`python3 -m py_compile tools/spec_fixtures.py`；`python3 tools/spec_fixtures.py check`；`cargo run -p scoop_tools -- spec-fixtures check`；临时 spec 的新旧 sync 输出 diff；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py tests/fixtures`（1532 checks）；`cargo run -p scoop -- test`（1532 checks）。（2026-05-26）
- [DONE] P1-T02R：复审 `tools/spec_fixtures.py` 与旧 `scoop_tools spec-fixtures` 实现，确认 `sync` / `check` / `check --fix` 的抽取、路径校验、重复路径、stale `.scoop` 清理和当前 spec doctest 输出语义等价；修复 Python 端对重复前导 `//` 注释对的 directive 解析，使其匹配旧 Rust `trim_start_matches("//")` 行为。验证：`python3 -m py_compile tools/spec_fixtures.py`；新旧临时 spec parity smoke（含 sync/check/check --fix/重复路径/非法路径/未闭合 fence/重复前导 `//`）；`python3 tools/spec_fixtures.py check`；`cargo run -q -p scoop_tools -- spec-fixtures check`；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py tests/fixtures`（1532 checks）；`cargo run -p scoop -- test`（1532 checks）。（2026-05-26）
- [DONE] P1-T03：新增 `tools/fixtures_matrix.py`，以 python 标准库平迁 `fixtures-matrix check` / `stdlib` 的 spec 章节 doctest 覆盖统计、`// FIXTURE:` / `EXPECT:` 头部解析、缺失/unknown 归类、stdlib 领域前缀矩阵与报告渲染；当前仓库 `check` 与 `stdlib` 输出和退出码均与旧 `scoop_tools fixtures-matrix` 完全一致。验证：`python3 -m py_compile tools/fixtures_matrix.py`；`python3 tools/fixtures_matrix.py check` / `stdlib` 分别与 `cargo run -q -p scoop_tools -- fixtures-matrix check` / `stdlib` diff 无差异；临时 spec + run-pass 目录的新旧 parity smoke；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py tests/fixtures`（1532 checks）；`cargo run -p scoop -- test`（1532 checks）。（2026-05-26）
- [DONE] P1-T03R：复审 `tools/fixtures_matrix.py` 与旧 `scoop_tools fixtures-matrix` 实现，确认 `check` / `stdlib` 的当前仓库输出完全一致，spec 章节解析、重复章节去重、重复 `//` 注释前缀、缺失 fixture 归类、未闭合 fenced code block 报错、run-pass stdlib 前缀矩阵与报告渲染语义等价；无需修改实现。验证：`python3 -m py_compile tools/fixtures_matrix.py`；当前仓库 `python3 tools/fixtures_matrix.py check` / `stdlib` 分别与 `cargo run -q -p scoop_tools -- fixtures-matrix check` / `stdlib` diff 无差异；临时 spec + run-pass 目录的新旧 parity smoke；未闭合 fenced code block 新旧均失败并报告 `未闭合`；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py tests/fixtures`（1532 checks）；`cargo run -p scoop -- test`（1532 checks）。（2026-05-26）
- [DONE] P1-T04：新增 `tools/safepoint_baseline.py`，以 python 标准库平迁 `scoop_tools safepoint-baseline` 的内置 workload 构建、LLVM IR `statepoint` / `gc-live` roots 统计与 Markdown 报告渲染；同步修复旧 Rust 工具引用已删除 async/task handoff fixture 的直接阻塞问题，改用当前可构建的 `safepoint_root_pressure_loop_basic.scoop` workload，并更新 safepoint baseline 文档快照。验证：`python3 -m py_compile tools/safepoint_baseline.py`；`cargo run -q -p scoop_tools -- safepoint-baseline` 与 `python3 tools/safepoint_baseline.py` 输出 diff 无差异；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py tests/fixtures`（1533 checks）；`cargo run -p scoop -- test`（1533 checks）。（2026-05-26）
- [DONE] P1-T04R：复审 `tools/safepoint_baseline.py` 与旧 `scoop_tools safepoint-baseline` 实现，确认内置 workload 列表、`O0` / `O2` 构建流程、LLVM IR statepoint 与 `gc-live` roots 统计、Markdown 报告渲染、stderr 输出与退出码语义均等价；无需修改实现。验证：`python3 -m py_compile tools/safepoint_baseline.py`；`cargo run -q -p scoop_tools -- safepoint-baseline` 与 `python3 tools/safepoint_baseline.py` 输出 diff 无差异；`cargo fmt --check`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py tests/fixtures`（1533 checks）；`cargo run -p scoop -- test`（1533 checks）。（2026-05-26）
- [DONE] P1-T05：新增 `tools/dependency_gate.py`，以 python 标准库平迁 `scoop_tools dependency-gate` 的 pipeline crate 依赖门禁与 source-boundary residual 扫描；依赖图改由 `cargo metadata --format-version 1` JSON 驱动，当前仓库输出与旧 Rust 版本完全一致。验证：`python3 -m py_compile tools/dependency_gate.py`；`python3 tools/dependency_gate.py` 与 `cargo run -q -p scoop_tools -- dependency-gate` 输出 diff 无差异；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py tests/fixtures`（1533 checks）；`cargo run -p scoop -- test`（1533 checks）。（2026-05-27）
