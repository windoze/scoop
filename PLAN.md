# Test Infrastructure Cleanup 执行计划

> 生成时间：2026-05-26
> 设计基线：[`TEST_INFRA_CLEANUP.md`](./TEST_INFRA_CLEANUP.md)
> 归档前置基线：
> - [`docs/archive/designs/PIPELINE_REFACTOR.md`](./docs/archive/designs/PIPELINE_REFACTOR.md)
> - [`docs/archive/designs/PIPELINE-CLEANUP.md`](./docs/archive/designs/PIPELINE-CLEANUP.md)
> - [`docs/archive/plans/PLAN-pipeline-refactor.md`](./docs/archive/plans/PLAN-pipeline-refactor.md)
> - [`docs/archive/plans/TODO-pipeline-refactor.md`](./docs/archive/plans/TODO-pipeline-refactor.md)
> 当前状态：P0~P4 待开始；上一轮 Pipeline Refactor（P0–P10）已全部完成并归档。

## 0. 目标

把 `scoop` / `scoopc` 中所有 “专为 self-test 而存在” 的代码彻底移除：fixture 运行引擎、`scoop test` / `scoopc test-fixtures` CLI、嵌入 `scoopc` 的仓库审计 `#[cfg(test)]` 模块、以及 `tools/scoop_tools/` Rust 工具箱。fixture 校验工作整体迁到 `tools/` 下的 python 脚本驱动；编译器与 driver 只把 `tests/fixtures/**/*.scoop` 当作普通源文件，不感知 “fixture” 概念。

`scoop test` 名字保留给后续 “项目级测试” 入口（类似 `cargo test`：发现 `Cone.toml` 下的测试项 → 编译 → 运行 → 汇报），与本计划无关；本计划只负责删除当前的 fixture-runner 实现，**不**为它引入占位或别名。

## 1. 硬约束

### 1.1 编译器与 driver 不持有 “fixture” 概念

1. `scoop` / `scoopc` 不得保留任何 `fixture` 命名的模块、CLI、env、API。
2. `tests/fixtures/**` 对编译器而言只是普通源文件，与用户在自己项目里写的 `*.scoop` 完全等价。
3. 外部 runner 唯一允许使用的入口是 [`TEST_INFRA_CLEANUP.md` §1](./TEST_INFRA_CLEANUP.md#1-编译器driver-对外暴露的-fixture-友好-命令面保留并固化) 列出的命令（`scoopc check-source` / `scoopc dump-*` / `scoopc emit-artifact` / `scoopc build-single-cone` / `scoopc link-cone` / `scoop build` / `scoop run`）；这些命令 stdout/stderr/exit-code 契约在本计划期间冻结。

### 1.2 不引入 Rust 测试工具

1. 新 fixture runner、迁出后的 audits、原 `scoop_tools` 4 个子命令的替代脚本，统一用 **python（仅标准库）** 实现，放在 `tools/` 下。
2. **不**复活 `tools/scoop_tools/`，**不**新增任何 Rust 工具 crate。
3. shell 脚本（`tools/*.sh`）仅用于 “逐 fixture 跑 + 收集日志” 的轻量编排，不实现指令解析 / golden 比对。

### 1.3 一次性切换，无兼容期

1. `scoop test` / `scoopc test-fixtures` / `cargo run -p scoop_tools -- ...` 一次性删除；不保留 deprecated alias、兼容 stub 或 “未知子命令时打印迁移提示” 等过渡逻辑。
2. CI / `tools/*.sh` / 文档与代码删除在同一轮 cleanup 内完成。

### 1.4 归档与历史记录不动

1. `docs/archive/**` 整体冻结；本计划不修改。
2. `TODO*-pipeline-refactor.md` / `PLAN-*.md` 等已归档文件中的 “验证通过：…” 行属于历史审计记录，不动。
3. 全仓 grep 时这些路径在白名单内。

### 1.5 fixture 目录与 golden 不重设计

1. `tests/fixtures/**` 目录结构、子目录命名约定（`*_multi_case` / `*_cone_case` / run-pass cone case 等）保持不变。
2. `EXPECT-*` 指令语法、`SYSROOT-DEPS:`、`RUN-MODE:`、`IGNORE-UNTIL-FIX:` 等语义照搬，新 runner 平迁实现，不重新设计。
3. 所有 golden 文件（`.hir` / `.mir` / `.effectfacts` / `.effectlowered` / `.scoopir.json` / `.stdout` / `.stderr` / `.sysroot/`）位置与字节级内容不变。
4. **例外（fixture-runner 自检 fixture 一并删除）**：依赖 `EXPECT-ERROR-CODE: scoop::fixtures::*`（即旧 fixture runner 自身定义的诊断码）的 fixture 不属于 “编译器/运行时行为测试”，本质是 fixture-runner 的 self-test，必须随旧 runner 一起删除。当前命中如下 4 条，全部不迁移到新 python runner：
   - `tests/fixtures/run-pass/timeout_should_fail.scoop`（`scoop::fixtures::run_exec_timeout`，依赖 `TIMEOUT:` 指令 + 10ms 触发）
   - `tests/fixtures/run-pass/exit_code_mismatch.scoop`（`scoop::fixtures::run_exit_code_mismatch`）
   - `tests/fixtures/run-pass/stderr_mismatch_distinguishable.scoop`（`scoop::fixtures::run_stderr_mismatch`）
   - `tests/fixtures/runtime_gc/gc_runner_stdout_mismatch_diagnostic_is_stable.scoop`（`scoop::fixtures::run_stdout_mismatch`）

   补充：盘点已确认 `scoopc` 内部**没有**专门为 `timeout_should_fail` 注入的后门（无 cfg(test) sleep / 无文件名特判 / 无 force-fail 注入）；`TIMEOUT:` 是 fixture runner 的通用机制，由 `crates/scoopc/src/fixtures/run_pass.rs` 的 `run_command_collect_output` / `wait_child_with_optional_timeout` 实现，将随 P3-T01 一起整体删除。

## 2. 现状摘要

详见 [`TEST_INFRA_CLEANUP.md` §2](./TEST_INFRA_CLEANUP.md#2-要删除的代码)。要点：

- **Fixture runner 引擎** ~6,150 行（`crates/scoopc/src/fixtures/{mod,expectations,run_pass}.rs` + `crates/scoopc/src/fixture_cli.rs`）。
- **CLI 表面 + dispatch**：`scoopc` 的 `test-fixtures` 子命令、`scoop` 的 `Command::Test` 变体 + `commands/test.rs`、`scoop` 与 `scoopc` `bin/main.rs` 内的相关分支。
- **嵌入 `scoopc` 的仓库审计 `#[cfg(test)]` 模块** ~2,080 行（`audit/spec_coverage.rs` + `pipeline_gap_audit.rs` + `pipeline_user_visible_failure_policy.rs`）。
- **`tools/scoop_tools/` Rust 工具箱** ~3,557 行（4 个子命令：`spec-fixtures` / `fixtures-matrix` / `safepoint-baseline` / `dependency-gate`）。
- **非归档目录调用串**：旧 `scoop test` 形态约 240 处、`cargo run -p scoop_tools -- ...` 约 147 处。

## 3. 阶段总览

| 阶段 | 主题 | 输出 |
|---|---|---|
| P0 | 现状冻结与契约盘点 | 文档化外部 runner 唯一可调用的编译器入口、`EXPECT-*` 指令清单、fixture 发现规则；确认 §1 命令面 stdout/stderr/exit-code 契约稳定；删除 fixture-runner 自检 fixture（§1.5.4）|
| P1 | 外部 python 脚本落地 | `tools/run_fixtures.py` + 4 个 `scoop_tools` 替代脚本 + 3 个 audit 替代脚本，跑出与旧实现等价的 pass/fail 集合 |
| P2 | CI / shell / 文档切换 | `.github/workflows/ci.yml`、`tools/*.sh`、`AGENTS.md`、`README.md`、`tools/README.md`、`PROMPT.md`、`SCOOP_FULL_SPEC.md` 等全部切到新入口 |
| P3 | 旧实现删除 | 删除 §2 列出的 ~12,000 行代码与 `tools/scoop_tools/` 整个 crate、`Cargo.toml` workspace 条目、相关 `cli.rs` 单测、`p8_docs_cleanup` 引用 |
| P4 | 残留搜索与验收 | 全仓 grep 清单无命中；新 runner 与所有迁出脚本验证通过；CI 跑通 |

依赖关系：P0 → P1 → P2 → P3 → P4。**P3 严格在 P1+P2 完成之后**，否则 CI 与开发流会断；P2 内部条目可与 P1 并行（python 脚本与文档替换可同 PR）。

## 4. 各阶段计划

### 4.1 P0：现状冻结与契约盘点

**目标**：把外部 runner 需要依赖的 “编译器对外契约” 显式列清，避免 P3 删除 fixture runner 时因为契约模糊导致回滚。

任务：

1. **P0-T01**：盘点 `EXPECT-*` 指令清单。从 `crates/scoopc/src/fixtures/expectations.rs` 抽出当前支持的所有指令名、参数语法、语义；落到 `docs/fixtures.md`（新文档）或 `tools/README.md` 内一节。
2. **P0-T02**：盘点 fixture 发现规则。把 `crates/scoopc/src/fixtures/mod.rs` 中的 phase router、`plan_targets`、`is_run_pass_cone_case_root` 等子目录约定整理为 python 可直接平迁的伪代码描述。
3. **P0-T03**：冻结编译器对外命令的 stdout/stderr/exit-code 契约。审视 `scoopc dump-*` / `emit-artifact` / `build-single-cone` / `link-cone` / `scoop build` / `scoop run` 当前输出，确认外部 runner 需要消费的字段都已稳定（如有需要补 round-trip 测试）。
4. **P0-T04**：删除 fixture-runner 自检 fixture（§1.5.4 列出的 4 条 `EXPECT-ERROR-CODE: scoop::fixtures::*` fixture），并复核 `scoopc` 内部确无为 `timeout_should_fail` 等自检 fixture 留下的 cfg(test) 旁路或文件名特判。删除时附带：
   - `tests/fixtures/run-pass/timeout_should_fail.scoop`
   - `tests/fixtures/run-pass/exit_code_mismatch.scoop`
   - `tests/fixtures/run-pass/stderr_mismatch_distinguishable.scoop`
   - `tests/fixtures/runtime_gc/gc_runner_stdout_mismatch_diagnostic_is_stable.scoop`

   验收：`grep -rn "scoop::fixtures::" tests/fixtures/ --include="*.scoop"` 无命中；旧 fixture runner 在剩余 fixture 集合上仍跑通（pass/fail 集合等价于本任务前的基线减去这 4 条，checks 计数对应下降）；scoopc 仓库内 `grep -rn "timeout_should_fail\|TIMEOUT_SHOULD_FAIL" --include="*.rs"` 无命中。

P0 不引入运行时（编译器/driver）行为变更；P0-T04 只删除 fixture 文件，编译器与 fixture runner 的 Rust 实现一律不动（fixture runner 的实际删除统一在 P3-T01）。

### 4.2 P1：外部 python 脚本落地

**目标**：在 `tools/` 下完成所有替代脚本，且与旧实现 pass/fail / 输出等价。仅依赖 python 标准库。

任务：

1. **P1-T00**：`scoopc check-source` —— 新增通用 phase-only 校验命令面（非 fixture API），覆盖 `parse` / `resolve` / `typecheck` / `infer`，支持单文件与 cone project 输入、`--source <path>` 选择项目内源文件、`--target-platform <id>`；契约写入 `docs/fixtures.md`。这是 `run_fixtures.py` 不使用 `dump-*` / `build-*` 工作绕过 typecheck-only 语义的前置条件。
2. **P1-T01**：`tools/run_fixtures.py` —— fixture runner。覆盖 phase 发现、`EXPECT-*` 指令解析、子进程驱动、golden 比对、多进程调度、超时/SIGKILL；调用对象只能是 §1.1 列出的编译器命令。验收：与旧 `scoop test` 在 `tests/fixtures/**` 上 pass/fail 集合与 checks 计数完全一致。
3. **P1-T02**：`tools/spec_fixtures.py {sync,check}` —— 替代 `scoop_tools spec-fixtures`。从 `SCOOP_FULL_SPEC.md` 抽取 `// FIXTURE:` 代码块，写入 / 比对 `tests/fixtures/spec_doctest/`。
4. **P1-T03**：`tools/fixtures_matrix.py {check,stdlib}` —— 替代 `scoop_tools fixtures-matrix`。
5. **P1-T04**：`tools/safepoint_baseline.py` —— 替代 `scoop_tools safepoint-baseline`。内部用 `cargo build` + `scoopc dump-stackmaps`。
6. **P1-T05**：`tools/dependency_gate.py` —— 替代 `scoop_tools dependency-gate`。建议用 `cargo metadata --format-version 1` JSON 输出。
7. **P1-T06**：`tools/audit_spec_coverage.py` —— 从 `crates/scoopc/src/audit/spec_coverage.rs` 平迁。
8. **P1-T07**：`tools/audit_pipeline_gap.py` —— 从 `crates/scoopc/src/pipeline_gap_audit.rs` 平迁。
9. **P1-T08**：`tools/audit_user_visible_failure_policy.py` —— 从 `crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 平迁。

每个任务的验收要求：与对应旧实现在相同输入下输出语义等价（exit code、stdout 关键字段、check 计数等）。脚本风格统一：`#!/usr/bin/env python3`、`from __future__ import annotations`、入口 `if __name__ == "__main__":`、错误经 `sys.exit(N)` 上报。

### 4.3 P2：CI / shell 编排 / 文档切换

**目标**：所有外部调用入口一次性切到 P1 新脚本。可与 P1 各任务在同一 PR 内捆绑，但必须在 P3 删除之前全部完成。

任务：

1. **P2-T01**：`.github/workflows/ci.yml` 切换。当前唯一 `scoop_tools` 调用点（第 51 行 `spec-fixtures check`）改为 `python3 tools/spec_fixtures.py check`；新增对 `python3 tools/run_fixtures.py` 的调用替换原 `cargo run -p scoop -- test`。
2. **P2-T02**：`tools/run_fixture_scan.sh` / `tools/run_run_pass_gc_scan.sh` / `tools/gc_microbench.sh` 内部调用串切换。
3. **P2-T03**：`AGENTS.md` 更新（[`TEST_INFRA_CLEANUP.md` §6](./TEST_INFRA_CLEANUP.md#6-文档更新) 列点）。
4. **P2-T04**：`README.md` 更新（同上）。
5. **P2-T05**：`tools/README.md` 整体重写为 python 脚本列表。
6. **P2-T06**：`PROMPT.md` 更新（fixture-suite 命令与超时说明）。
7. **P2-T07**：`SCOOP_FULL_SPEC.md` 更新（spec 内 `cargo run -p scoop -- test` / `scoop test` 调用串）。
8. **P2-T08**：`tests/fixtures/umb_fix/B-15-when-pattern/_README.md` 与其他 `tests/fixtures/**/_README.md` 内的旧调用串替换；其他 fixture README 实施时一并 grep 一遍。
9. **P2-T09**：`docs/safepoint_baseline.md` 内的 `safepoint-baseline` 调用切换。

P2 完成后，仓库内所有非归档调用应已不再使用旧入口，但旧入口的 Rust 实现仍存在；CI 在切换后应仍能跑通。

### 4.4 P3：旧实现删除

**目标**：删除所有专为 self-test 而存在的 Rust 代码与 `tools/scoop_tools/`。

任务：

1. **P3-T01**：删除 `crates/scoopc/src/fixtures/{mod,expectations,run_pass}.rs` 与 `crates/scoopc/src/fixture_cli.rs`。
2. **P3-T02**：删除 `scoopc` `driver_cli.rs` 中 `CompilerCli::TestFixtures` 变体、`Some("test-fixtures")` 路由、`parse_test_fixtures` 实现与 USAGE 文本；删除 `crates/scoopc/src/bin/scoopc.rs` 内对应 dispatch；删除 `crates/scoopc/src/lib.rs` 内 `pub mod fixtures` / `pub mod fixture_cli` 导出。
3. **P3-T03**：删除 `scoop` `cli.rs` 内 `Command::Test { .. }` 整个变体与所有 `test_command_parses_*` 单测；删除 `crates/scoop/src/commands/test.rs`；删除 `crates/scoop/src/commands/mod.rs` 内 dispatch 分支与 `pub mod test`。
4. **P3-T04**：迁出 §1.6 已说明的三个审计模块后，删除 `crates/scoopc/src/audit/{mod,spec_coverage}.rs`、`crates/scoopc/src/pipeline_gap_audit.rs`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`；删除 `lib.rs` 第 99 / 102 行 `#[cfg(test)] mod` 挂载点。
5. **P3-T05**：删除 `tools/scoop_tools/` 整个 crate；从 workspace 根 `Cargo.toml` 第 24 行删除 `"tools/scoop_tools",`。
6. **P3-T06**：清理 `crates/scoop/tests/p8_docs_cleanup.rs` 第 56 行对 `tools/scoop_tools/src/fixtures_matrix.rs` 的源路径引用（如果该测试只为这条引用存在则整体删除；否则按 P1-T03 / P3-T05 的实际位置更新）。
7. **P3-T07**：清理 P3-T01 / P3-T04 删除后产生的死代码、孤立常量、`SCOOP_FIXTURE_*` env 名字常量等；按 [`TEST_INFRA_CLEANUP.md` §2.6](./TEST_INFRA_CLEANUP.md#26-编译器内部为-fixture-服务而留的旁路--hooks) 的清单 grep。

P3 完成后 `cargo build` 与 `cargo test --all --all-targets` 必须通过；`cargo run -p scoop -- test` / `cargo run -p scoopc -- test-fixtures` / `cargo run -p scoop_tools -- ...` 应全部报 “未知子命令” 或 “未知 package”。

### 4.5 P4：残留搜索与验收

**目标**：最终验证。

任务：

1. **P4-T01**：按 [`TEST_INFRA_CLEANUP.md` §7 步骤 5](./TEST_INFRA_CLEANUP.md#7-实施顺序建议) 列出的所有 token 全仓 grep，确认非归档目录无命中（白名单：`docs/archive/**` 与 `TODO*-pipeline-refactor.md` / `PLAN-pipeline-refactor.md` 内历史 “验证通过：…” 行）。
2. **P4-T02**：`cargo metadata --format-version 1 | grep scoop_tools` 无命中。
3. **P4-T03**：`python3 tools/run_fixtures.py` 与旧 `scoop test` 在最近一份基线（commit 时间点）上 pass/fail 集合 + checks 计数一致；如有 diff，必须在 P4 结束前定位并修复。
4. **P4-T04**：`python3 tools/{spec_fixtures,fixtures_matrix,safepoint_baseline,dependency_gate,audit_*}.py` 全部跑通；`dependency-gate` 规则与旧 Rust 版本完全等价（输出可形态不同，但同一个仓库状态下结论一致）。
5. **P4-T05**：CI 在切换后跑通；本地 `cargo fmt` / `cargo clippy --all-targets -- -D warnings` / `cargo test --all --all-targets` 通过。
6. **P4-T06**：`tools/run_fixture_scan.sh` / `tools/run_run_pass_gc_scan.sh` 用新入口跑通。

## 5. 验收标准

cleanup 整体完成的判据，与 [`TEST_INFRA_CLEANUP.md` §8](./TEST_INFRA_CLEANUP.md#8-验证清单) 等价：

1. `crates/scoopc/src/{fixtures,fixture_cli.rs,audit,pipeline_gap_audit.rs,pipeline_user_visible_failure_policy.rs}` 全部不再存在。
2. `crates/scoop/src/commands/test.rs` 与 `Command::Test` 变体不再存在；`scoop test` 报未知子命令。
3. `tools/scoop_tools/` 不再存在；`Cargo.toml` workspace 不再含该成员；`cargo metadata` 不再列出该 crate。
4. `tools/` 下存在 `run_fixtures.py` + `spec_fixtures.py` + `fixtures_matrix.py` + `safepoint_baseline.py` + `dependency_gate.py` + `audit_spec_coverage.py` + `audit_pipeline_gap.py` + `audit_user_visible_failure_policy.py`，单独可运行，仅依赖 python 标准库。
5. 全仓 grep 清单（[`TEST_INFRA_CLEANUP.md` §7 步骤 5](./TEST_INFRA_CLEANUP.md#7-实施顺序建议)）在 §1.4 白名单外无命中。
6. CI 通过；`cargo test --all --all-targets` 通过；新 runner 与旧 runner pass/fail 等价。

## 6. 说明

- 本计划只覆盖 “删除自测代码 + 外部脚本接管” 这一件事；未来项目级 `scoop test`（cargo-test 风格）的设计不在本计划范围。
- 阶段内 task 编号风格为 `Px-Tnn`（与已归档的 Pipeline Refactor 系列一致）；具体任务体在实施时按需补 `TODO.md` / `TODO-N.md`，本计划完成时并不要求拆出独立 TODO 文件。
- 本计划完成后，`PLAN.md` 与 `TEST_INFRA_CLEANUP.md` 一并归档到 `docs/archive/`，命名建议：`PLAN-test-infra-cleanup.md` / `TEST_INFRA_CLEANUP.md`（移到 `docs/archive/designs/`）。
