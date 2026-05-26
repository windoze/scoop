# TEST_INFRA_CLEANUP

把 `fixture` 概念从 `scoop` / `scoopc` 中拿掉，改由外部脚本驱动；同时删除 `scoop test` 子命令，为它将来作为 “cargo test 风格的项目级测试入口” 腾出位置。

## 0. 总体原则

- **`scoop` / `scoopc` 不再持有 “fixture” 这个概念**。对编译器和 driver 而言，`tests/fixtures/**/*.scoop` 只是普通的源文件，与用户在自己项目里写的 `*.scoop` 完全等价。
- **fixture 校验工作整体下放到 shell/python 脚本**，统一放在 `tools/` 下。**不**再使用 Rust 写的工具箱（包括现有的 `tools/scoop_tools/`，见 §2.6）。理由：所有 fixture / audit / 覆盖矩阵 / 依赖门禁工作的实质都是 “跑 cargo 子命令 + 解析文本”，python（标准库即可）和 shell 写最直接，不需要 cargo 编译开销，也不需要 clap/miette 这类依赖。
- **`scoop test` 子命令删除**。后续要重新引入的 `scoop test` 将是 “项目级测试” 入口（类似 `cargo test`：发现 `Cone.toml` 下的测试项 → 编译 → 运行 → 汇报），与当前的 fixture-runner 没有任何关系。本文件覆盖范围之内不会保留 “临时占位实现” 或 deprecated alias。
- **不做兼容期**。`scoop test` / `scoopc test-fixtures` / `cargo run -p scoop_tools -- ...` 一次性切到新入口；旧 CI / 脚本 / 文档同步更新。

非目标（本次不做）：
- 不重新设计 fixture expectation 语法（`EXPECT-*` 指令），现有指令语义照搬到外部 runner。
- 不修改 `tests/fixtures/**` 的目录结构与 golden 文件。
- 不在本次内引入新的 `scoop test`（项目级）实现 —— 只腾出名字。

## 1. 编译器/driver 对外暴露的 “fixture 友好” 命令面（保留并固化）

外部 runner 唯一被允许使用的 scoop/scoopc 入口（这些命令不知道 “fixture” 的存在，对它们而言输入就是普通 `.scoop` 文件 / cone 目录）：

| 用途 | 命令 |
|---|---|
| AST/HIR/MIR/IR/EffectFacts/EffectLowered dump | `scoopc dump-ast` / `dump-hir` / `dump-mir` / `dump-ir` / `dump-effect-facts` / `dump-effect-lowered` |
| 前端 phase-only 校验 | `scoopc check-source --phase {parse,resolve,typecheck,infer} --input <file-or-cone-dir> [--source <path>] [--target-platform <id>]` |
| RTTI dump | `scoopc dump-rtti` |
| Stackmap dump / 校验 | `scoopc dump-stackmaps [--verify-roots] [--dump-records]` |
| 单文件 emit artifact | `scoopc emit-artifact --kind {llvm-ir,obj,asm}` |
| 编译单 cone（DAG scheduler 子进程） | `scoopc build-single-cone` |
| Link cone | `scoopc link-cone` |
| 端到端 build | `scoop build` |
| 端到端 build + run | `scoop run` |

除 P1-T00 新增的通用 `check-source` phase-only 校验入口外，这些命令在本次 cleanup 内**不删不改**，只确认它们的 stdout/stderr/exit-code 契约稳定，外部 runner 通过它们驱动 fixture，**不再**通过任何 `test-fixtures` 模式。`check-source` 不得持有 “fixture” 概念；它只按普通源码 / 普通 cone project 输入运行指定前端阶段，成功时 stdout 为空，失败时沿用结构化 stderr 诊断与非 0 exit code。

## 2. 要删除的代码

### 2.1 `scoopc` 中的 fixture runner 引擎（约 6,150 行）

| 文件 | 行数 | 处置 |
|---|---:|---|
| `crates/scoopc/src/fixtures/mod.rs` | 3,805 | **整体删除**（phase router、`plan_targets`、`run_all`、各 phase runner、~30 个 `Diagnostic` 错误类型、sysroot overlay 发现、retired `#[cfg(any())]` 残留） |
| `crates/scoopc/src/fixtures/expectations.rs` | 421 | **整体删除**（`EXPECT-*` 指令解析器；语义迁移到外部 runner） |
| `crates/scoopc/src/fixtures/run_pass.rs` | 1,426 | **整体删除**（执行子进程 + stdout/stderr golden + dump-stackmaps 模式 + timeout/SIGKILL） |
| `crates/scoopc/src/fixture_cli.rs` | 377 | **整体删除**（`FixtureCliOptions`、多进程调度、`SCOOP_FIXTURE_WORKER` / `SCOOP_FIXTURE_OK=` 协议） |

### 2.2 `scoopc` CLI 表面与 dispatch

| 位置 | 处置 |
|---|---|
| `crates/scoopc/src/driver_cli.rs` 第 49 行 `CompilerCli::TestFixtures(..)` 变体 | **删除** |
| `crates/scoopc/src/driver_cli.rs` 第 199 行 `Some("test-fixtures") => parse_test_fixtures(..)` 路由 | **删除** |
| `crates/scoopc/src/driver_cli.rs` 第 546 行 `parse_test_fixtures` 实现 | **删除** |
| `crates/scoopc/src/driver_cli.rs` 第 29–30 行 USAGE 文档 | **删除 `test-fixtures` 部分** |
| `crates/scoopc/src/bin/scoopc.rs` 第 28 行 `CompilerCli::TestFixtures(sub) => scoopc::fixture_cli::run(sub)` | **删除** |
| `crates/scoopc/src/lib.rs` 中 `pub mod fixtures;` / `pub mod fixture_cli;` 等导出 | **删除** |

### 2.3 `scoop` 中的 `test` 子命令（用户要求一并删除，名字保留给未来项目级测试入口）

| 位置 | 处置 |
|---|---|
| `crates/scoop/src/cli.rs` 第 26–60 行 `Command::Test { .. }` 整个变体 | **删除** |
| `crates/scoop/src/cli.rs` 中所有 `test_command_parses_*` 单测 | **删除** |
| `crates/scoop/src/commands/test.rs` 整个文件（125 行） | **删除** |
| `crates/scoop/src/commands/mod.rs` 中 `Command::Test { .. } => test::run(..)` dispatch 分支 + `pub mod test;` | **删除** |

> 注意：`scoop test` 这个命令名本身不引入任何占位/弃用实现。新版 `scoop test`（项目级）将由后续独立任务重新引入，本次只“腾位置”。

### 2.4 嵌在 `scoopc` 里的仓库审计 `#[cfg(test)]` 模块（约 2,080 行）

这些模块的本质是 “读 markdown / csv / 源码并 grep”，不依赖编译器内部 API，应当作为外部 lint 脚本：

| 文件 | 行数 | 处置 |
|---|---:|---|
| `crates/scoopc/src/audit/spec_coverage.rs` | 608 | **整体迁出**到 `tools/audit_spec_coverage.py`，同时删除 `crates/scoopc/src/audit/mod.rs` |
| `crates/scoopc/src/audit/mod.rs` | 3 | **删除** |
| `crates/scoopc/src/pipeline_gap_audit.rs` | 328 | **整体迁出**到 `tools/audit_pipeline_gap.py` |
| `crates/scoopc/src/pipeline_user_visible_failure_policy.rs` | 1,142 | **整体迁出**到 `tools/audit_user_visible_failure_policy.py` |
| `crates/scoopc/src/lib.rs` 第 99 / 102 行 `#[cfg(test)] mod pipeline_gap_audit;` 与 `pipeline_user_visible_failure_policy;` 挂载点 | **删除** |

迁出形态为 **python 脚本**（见 §2.6）：

| 旧（在 `scoopc` 内 `#[cfg(test)]`） | 新（python） |
|---|---|
| `audit/spec_coverage.rs` | `tools/audit_spec_coverage.py` |
| `pipeline_gap_audit.rs` | `tools/audit_pipeline_gap.py` |
| `pipeline_user_visible_failure_policy.rs` | `tools/audit_user_visible_failure_policy.py` |

CI 用 `python3 tools/audit_*.py` 直接调用，替换 `cargo test -p scoopc` 中包含这三块的部分。

### 2.5 `tools/scoop_tools/` Rust 工具箱整体移除（约 3,557 行）

`scoop_tools` 现有 4 个子命令本质都是 “跑 cargo / 读文件 / 解析文本”，没有任何编译器内部 API 依赖，用 shell+python 写更短更直接，且不必每次为它编译一份 Rust 二进制。**整个 crate 删除**。

| 现有子命令 | 文件 | 行数 | 实质 | 替代脚本（建议命名） |
|---|---|---:|---|---|
| `spec-fixtures sync` / `spec-fixtures check` | `tools/scoop_tools/src/spec_fixtures/{mod,parse}.rs` | 428 | 从 `SCOOP_FULL_SPEC.md` 抽取带 `// FIXTURE:` 标记的 fenced code block，写入 / 比对 `tests/fixtures/spec_doctest/` | `tools/spec_fixtures.py {sync,check}` |
| `fixtures-matrix check` / `fixtures-matrix stdlib` | `tools/scoop_tools/src/fixtures_matrix.rs` | 630 | 解析 spec 章节标题，统计 fixture 覆盖（每章至少 1 pass + 1 fail）；stdlib 模式按域分组 | `tools/fixtures_matrix.py {check,stdlib}` |
| `safepoint-baseline` | `tools/scoop_tools/src/safepoint_baseline.rs` | 303 | 跑内置 workload 的 `cargo build` + `scoopc dump-stackmaps`，汇总 statepoint / gc-live roots 指标 | `tools/safepoint_baseline.py` |
| `dependency-gate` | `tools/scoop_tools/src/dependency_gate.rs` | 2,072 | 用 `cargo tree` / `cargo metadata` 校验 base / fact / stage / cone / driver crates 的依赖方向 | `tools/dependency_gate.py`（推荐用 `cargo metadata --format-version 1` 的 JSON 输出） |
| `main.rs`（clap dispatch） | `tools/scoop_tools/src/main.rs` | 124 | — | （没有对应物，每个 python 脚本直接是入口） |

**实施清单**：

- 删除整个 `tools/scoop_tools/` 目录。
- `Cargo.toml` 第 24 行 workspace member 条目 `"tools/scoop_tools"` 删除。
- `crates/scoop/tests/p8_docs_cleanup.rs` 第 56 行对 `tools/scoop_tools/src/fixtures_matrix.rs` 的源文件路径引用一并清理（如果该测试在 §2.4 迁出后还存在，按 §2.4 的迁出位置更新；如果整个测试只是为这条引用存在，则删除）。
- `.github/workflows/ci.yml` 第 51 行 `cargo run -p scoop_tools -- spec-fixtures check` → `python3 tools/spec_fixtures.py check`（CI 当前唯一的 `scoop_tools` 调用点）。
- 全仓非归档 `cargo run -p scoop_tools -- ...` 调用（约 147 处）按上表逐项替换。

### 2.6 编译器内部为 fixture 服务而留的旁路 / hooks

下面这些是排查时已知与 fixture runner 强耦合的地方，删除 fixtures 后应一并清理（具体清单在实施时由 grep `crate::fixtures::` / `crate::fixture_cli::` / `SCOOP_FIXTURE_*` 落地）：

- `SCOOP_FIXTURE_WORKER` / `SCOOP_FIXTURE_OK=` / `SCOOP_FIXTURE_SCOOP_BIN` 等 env 名字常量、`apply_session_options_to_command`、`current_scoopc_exe_path`、`RunPassEnvOverrides`、`PlannedFixtureTarget`、`is_run_pass_cone_case_root`、`FixturePhase` 等所有公开/内部 item 全部消失。
- 任何 `cfg(test)` 之外通过 `crate::fixtures::*` 引用的 helper（如有），随其消费方一起处理。

## 3. 外部 runner 落地形态

**最终形态：纯 python 脚本，放在 `tools/` 下，仅依赖 python 标准库**。不引入新的 Rust 工具 crate，也不复活 `scoop_tools`。

建议入口：`tools/run_fixtures.py`（命名可调整，但必须落在 `tools/` 下、是 python 脚本）。

职责切分：

- **调度（多进程并行 / 失败聚合 / pass/fail 计数）**：python，使用标准库 `concurrent.futures` 或 `multiprocessing`。
- **fixture 发现（含 `*_multi_case` / `*_cone_case` / run-pass cone case 等子目录约定）**：python；规则从原 `crates/scoopc/src/fixtures/mod.rs` 中的 `plan_targets` / `is_run_pass_cone_case_root` 等函数平迁。
- **`EXPECT-*` 指令解析（`EXPECT:` / `EXPECT-ERROR-CODE:` / `RUN-STDOUT:` / `BUILD-LLVM-REGEX:` / `IGNORE-UNTIL-FIX:` / `RUN-MODE:` / `SYSROOT-DEPS:` / `EXPECT-MONOMORPH-HIT:` / `RUN-STACKMAPS-RECORDS-GT:` …）**：python；每条指令的语法和语义照搬 `crates/scoopc/src/fixtures/expectations.rs`。
- **golden 比对（`.hir` / `.mir` / `.effectfacts` / `.effectlowered` / `.scoopir.json` / `.stdout` / `.stderr`）**：python；当前是字节级 / 行级 diff，python 标准库 `difflib` 足够。
- **子进程驱动**：python `subprocess.run`，调用对象只能是 §1 列出的 `scoopc check-source` / `scoopc dump-*` / `scoopc emit-artifact` / `scoop build` / `scoop run` —— 这些命令对外完全不知道 “fixture” 概念。

`tools/run_fixture_scan.sh` / `tools/run_run_pass_gc_scan.sh` / `tools/gc_microbench.sh` 保留为轻量 shell 编排，内部调用对象由 `scoop test --fixtures <fixture>` 一律切换为 `python3 tools/run_fixtures.py <fixture>`。

**为什么不用 Rust**：见 §0 与 §2.5。所有动作都是 “跑 cargo + 解析文本 + 写文件”；用 python 标准库写大概率不超过 1500 行，避免每次跑都重新 cargo build 一份工具二进制，也避免引入 `clap` / `miette` / `tempfile` 这类依赖。

## 4. fixture 目录与 golden 的处理

- `tests/fixtures/**` 与所有 `.hir` / `.mir` / `.effectfacts` / `.effectlowered` / `.scoopir.json` / `.stdout` / `.stderr` / `.sysroot/` 目录**位置不变**。
- `EXPECT-*` 指令、`SYSROOT-DEPS:` 语义、`RUN-MODE:`、`IGNORE-UNTIL-FIX:` 等保持现状，由新 runner 重新解析（语义上是平迁，不是重设计）。
- run-pass cone case / resolve-multi / typecheck-multi / typecheck-cone / run-pass-cone 等子目录命名约定（即原 `is_run_pass_cone_case_root` 等判定）保持不变；新 runner 复制这套发现规则。

## 5. CI / 验证命令切换

旧 → 新（一次性切换；不保留 alias）。

**注意：旧 `scoop test` 在仓库里有多种等价写法，下面任何一种都属于待替换范围**（即整组 “fixture runner via scoop/scoopc 二进制” 的 invocation 形态）：

- `scoop test ...`
- `cargo run -p scoop -- test ...`（最常见，TODO-* / 验证记录里大量出现）
- `cargo run -p scoop --features llvm -- test ...`
- `target/debug/scoop test ...` / `target/release/scoop test ...`（`tools/*.sh` 与 docstring 里出现）
- `scoopc test-fixtures ...`
- `cargo run -p scoopc -- test-fixtures ...`

仓库内 `docs/archive/**` 之外这类 invocation 现有 ~240 处，迁移期间需要全部清理。

**fixture runner 切换**：

| 旧命令（任一写法） | 新命令 |
|---|---|
| `… scoop test` / `… scoop -- test` | `python3 tools/run_fixtures.py` |
| `… scoop test --fixtures <path>` / `… scoop -- test --fixtures <path>` | `python3 tools/run_fixtures.py <path>` |
| `… scoop test --gc-stress --gc-move --threads N --processes M` 等开关组合 | `python3 tools/run_fixtures.py --gc-stress --gc-move --threads N --processes M` |
| `scoopc test-fixtures ...` / `cargo run -p scoopc -- test-fixtures ...` | （删除，不再暴露） |

**`scoop_tools` 子命令切换**（同步随 §2.5 一起执行）：

| 旧命令 | 新命令 |
|---|---|
| `cargo run -p scoop_tools -- spec-fixtures sync` | `python3 tools/spec_fixtures.py sync` |
| `cargo run -p scoop_tools -- spec-fixtures check` | `python3 tools/spec_fixtures.py check` |
| `cargo run -p scoop_tools -- fixtures-matrix check` | `python3 tools/fixtures_matrix.py check` |
| `cargo run -p scoop_tools -- fixtures-matrix stdlib` | `python3 tools/fixtures_matrix.py stdlib` |
| `cargo run -p scoop_tools -- safepoint-baseline` | `python3 tools/safepoint_baseline.py` |
| `cargo run -p scoop_tools -- dependency-gate` | `python3 tools/dependency_gate.py` |

`tools/run_fixture_scan.sh` / `tools/run_run_pass_gc_scan.sh` / `tools/gc_microbench.sh` 内部调用串同步替换。`.github/workflows/ci.yml` 内 `scoop_tools` 调用同步替换（CI 当前唯一调用点是 `spec-fixtures check`）。

## 6. 文档更新

下面列出**必须**同步修改的文档及具体改点。所有提到 “`scoop test`”、“`scoop test --fixtures`”、“`scoopc test-fixtures`”、“fixture runner 在 scoop/scoopc 内部” 之类的描述都要改写。

**普适规则**：

- 所有非归档文档里出现的 `scoop test` / `cargo run -p scoop -- test` / `cargo run -p scoop --features llvm -- test` / `target/debug/scoop test` / `scoopc test-fixtures` / `cargo run -p scoopc -- test-fixtures` 一律替换为对应的 `python3 tools/run_fixtures.py` 形式（开关位置整体平移）。
- 所有非归档文档里出现的 `cargo run -p scoop_tools -- <子命令>` 按 §5 第二张表逐项替换为对应的 `python3 tools/<script>.py` 形式。
- 文档里凡描述 “Rust 工具箱 `tools/scoop_tools/`” 的语句改为 “python 脚本 `tools/*.py`”。

下面列出的是**已确认必改**的文件，但实施时需以 §7 中的 grep 清单为准做最终扫描。

- **`AGENTS.md`**
  - 第 5 行 “driver CLI (`scoop`) used to run fixtures and (optionally) build/run programs” → 改为 “driver CLI (`scoop`) used to build/run programs”，去掉 “run fixtures”。
  - 第 11 行 “`tools/scoop_tools/` + `tools/*.sh`: repo utilities” → 改为 “`tools/*.py` + `tools/*.sh`: repo utilities (fixture runner, spec sync, audits, dependency gate, …)”，并去掉所有 “Rust 工具箱” 措辞。
  - 第 22–23 行 spec-fixtures 示例 `cargo run -p scoop_tools -- spec-fixtures check` → 替换为 `python3 tools/spec_fixtures.py check`。
  - 第 25–26 行示例 `cargo run -p scoop -- test` → 替换为 `python3 tools/run_fixtures.py`。
  - 第 47 行 `cargo run -p scoop_tools -- spec-fixtures sync` → 替换为 `python3 tools/spec_fixtures.py sync`。
  - “Testing Guidelines” 段落补一句：fixture 套件由 `tools/run_fixtures.py` 驱动；`scoop` / `scoopc` 不再内置 fixture runner，也不再使用 `scoop_tools` Rust 工具箱。

- **`README.md`**
  - 第 71 行 “只构建前端/中端与 fixtures runner 的‘无后端模式’” 改写为 “只构建前端/中端的‘无后端模式’”（fixture runner 不再属于 scoop 二进制）。
  - 第 94–97 行 “跑 fixtures” 示例 `cargo run -p scoop -- test` → `python3 tools/run_fixtures.py`。
  - 第 183 行附近的 “tests/fixtures/” 描述里强调 fixture 的执行方为 `tools/run_fixtures.py`（python 脚本）。

- **`PROMPT.md`**
  - 第 110–113 行 “Full fixture-suite runs … `cargo run -p scoop -- test`” 替换为新命令。
  - 全文搜索 “scoop test” / “fixture suite” / “`scoop` 跑 fixtures” 类描述并替换。

- **`tools/README.md`**
  - **整体重写**：删除 “Rust 工具箱 `tools/scoop_tools/`” 一节及其 4 个子命令说明，替换为 python 脚本列表：
    - `tools/run_fixtures.py`：fixture 套件驱动（取代 `scoop test`）
    - `tools/spec_fixtures.py {sync,check}`：spec doctest fixture 同步/校验
    - `tools/fixtures_matrix.py {check,stdlib}`：覆盖矩阵报告
    - `tools/safepoint_baseline.py`：safepoint / gc-live roots 基线
    - `tools/dependency_gate.py`：crate 依赖方向门禁
    - `tools/audit_spec_coverage.py` / `tools/audit_pipeline_gap.py` / `tools/audit_user_visible_failure_policy.py`：从 `scoopc` 内迁出的仓库审计
  - 第 12 行 “默认用 `target/debug/scoop test --fixtures <fixture>` 逐条执行” 改为 “默认用 `python3 tools/run_fixtures.py <fixture>` 逐条执行”。

- **`tools/run_fixture_scan.sh` / `tools/run_run_pass_gc_scan.sh`**
  - 内部调用串、`command_shape` 字符串、`Usage:` 段示例全部把 `scoop test --fixtures` 换成新入口。
  - 默认 `SCOOP_BIN` 对应位置改为直接调用 `python3 tools/run_fixtures.py`（不再需要找 scoop 二进制）。

- **`PIPELINE_REFACTOR.md` / `PIPELINE-CLEANUP.md` / `SCOOP_RUNTIME.md` / `SCOOP_FULL_SPEC.md` 等长篇设计文档**
  - 全文搜索 `scoop test`、`test-fixtures`、“fixture runner” 等；只要文中描述的是 “scoop 内置 fixture 套件”，统一改为 “外部 fixture runner（`tools/run_fixtures.py`）”。
  - 引用具体命令的段落同 §5 表格替换。

- **`SCOOP_FULL_SPEC.md`**
  - 全文搜 `scoop test` / `cargo run -p scoop -- test` / `target/debug/scoop test`，按普适规则替换。Spec 文档里出现的 fixture 调用串通常是给读者演示 “怎么跑这条规则对应的 fixture”，全部改写为新入口。

- **`tests/fixtures/umb_fix/B-15-when-pattern/_README.md`**
  - 内含旧形式 invocation 命令，需替换。其他 `tests/fixtures/**/_README.md` 在实施时也要 grep 一遍。

- **`TODO.md` / `TODO-1.md` … `TODO-7.md` / `PLAN.md`**
  - **不动历史 “验证通过：…” 行**（那是已完成任务的运行记录，保持原样作为审计痕迹）；这意味着这批文件中绝大多数旧形式 invocation 都属于 “历史记录” 不动。
  - **要动的是 “未来要执行” 的命令串**：仍未 `[DONE]` 的任务里如果写着 `cargo run -p scoop -- test ...` 作为验收命令，需更新到新入口。
  - 在 `TODO.md` 顶部 / `TODO-7.md` 当前正在推进章节内新增一项任务：本 cleanup 的执行任务（编号留空，由实施时分配），引用本文件作为方案。
  - `TODO-7.md` 第 1722 行附近 “compiler tooling 迁出 facade” 段落里关于 “`scoop test` 只包装 `scoopc test-fixtures`” 的描述需要标注 “已废弃，本任务由 `TEST_INFRA_CLEANUP.md` 取代”。

- **`Cargo.toml`（workspace 根）**
  - 第 24 行 `"tools/scoop_tools",` 删除。

- **`.github/workflows/ci.yml`**
  - 第 51 行 `cargo run -p scoop_tools -- spec-fixtures check` → `python3 tools/spec_fixtures.py check`。
  - 全文 grep `scoop_tools` / `scoop test` / `cargo run -p scoop -- test` 同步替换。

- **`crates/scoop/tests/p8_docs_cleanup.rs`**
  - 第 56 行对 `tools/scoop_tools/src/fixtures_matrix.rs` 的源路径引用清理（该测试如果只是为这条引用存在，整体删除；如果还覆盖其他路径，按 §2.4 / §2.5 迁出后的实际位置更新）。

- **`docs/safepoint_baseline.md`**
  - 出现的 `cargo run -p scoop_tools -- safepoint-baseline` 改为 `python3 tools/safepoint_baseline.py`。

- **`docs/archive/**`**
  - **不修改**。归档目录是历史快照，保留旧术语。

- **新文档（可选）**
  - 若新 runner 支持的指令集合需要单独说明（`EXPECT-*`、`RUN-MODE:` 等），新增 `docs/fixtures.md`（或在 `tools/README.md` 内单独一节），把当前散落在 `crates/scoopc/src/fixtures/expectations.rs` 顶部注释里的语法整理成一份可读文档。这件事可以与代码迁移同 PR，也可以拆出后续小任务，但不能因此拖延 §2 的删除。

## 7. 实施顺序建议

不做强约束，但推荐：

1. 先在 `tools/` 下落地 python 版新 runner（`tools/run_fixtures.py`，覆盖到现有所有 phase + `EXPECT-*` 语法 + golden 比对 + 多进程调度），用现成 `tests/fixtures/**` 验证与旧 runner 输出等价（pass/fail 集合、checks 计数一致）。同期把 §2.5 列出的 `scoop_tools` 4 个子命令也用 python 改写到 `tools/` 下。
2. 切换 CI / `tools/*.sh` / 文档到新入口（python 脚本）。
3. 删除 `crates/scoopc/src/fixtures/`、`crates/scoopc/src/fixture_cli.rs`、`scoopc` 的 `test-fixtures` CLI、`scoop test` 子命令、`commands/test.rs`、相关 cli.rs 单测；**同步删除 `tools/scoop_tools/` 整个 crate 与 `Cargo.toml` workspace 成员条目**。
4. 迁出 §2.4 三个审计模块（python 形态）。
5. 全仓 grep 以下 token 确认非归档目录无残留（`docs/archive/**`、以及 `TODO*.md` / `PLAN*.md` 内的历史 “验证通过：…” 行除外）：
   - `scoop test`（覆盖 `scoop test`、`scoop test --fixtures`、`scoop test --gc-stress` 等所有变体）
   - `cargo run -p scoop -- test` / `cargo run -p scoop --features llvm -- test`
   - `target/debug/scoop test` / `target/release/scoop test`
   - `scoopc test-fixtures` / `cargo run -p scoopc -- test-fixtures`
   - `SCOOP_FIXTURE_WORKER` / `SCOOP_FIXTURE_OK` / `SCOOP_FIXTURE_SCOOP_BIN`
   - `crate::fixtures::` / `crate::fixture_cli::` / `crate::audit::`
   - `pipeline_gap_audit` / `pipeline_user_visible_failure_policy`
   - `FixturePhase` / `PlannedFixtureTarget` / `FixtureCliOptions` / `FixtureExpectation` / `RunPassEnvOverrides`
   - `scoop_tools` / `scoop-tools`（包括 `cargo run -p scoop_tools`、`tools/scoop_tools/` 路径引用、workspace 成员、Cargo.toml 引用）
   - 旧子命令名 `spec-fixtures` / `fixtures-matrix` / `safepoint-baseline` / `dependency-gate`（在替换为 `python3 tools/*.py` 后，原写法应只可能出现在归档/历史验证记录中）

## 8. 验证清单

cleanup 完成后必须满足：

- `cargo build -p scoop -p scoopc --features llvm` 通过，`crates/scoopc/src/fixtures/` / `fixture_cli.rs` / `audit/` / `pipeline_gap_audit.rs` / `pipeline_user_visible_failure_policy.rs` / `tools/scoop_tools/` 全部不再存在。
- `cargo metadata --format-version 1 | grep scoop_tools` 无命中（workspace 不再含该 crate）。
- `cargo run -p scoop -- test` / `cargo run -p scoop --features llvm -- test` / `target/debug/scoop test` 全部报 “未知子命令”（`scoop test` 已删除），`scoopc test-fixtures` / `cargo run -p scoopc -- test-fixtures` / `cargo run -p scoop_tools -- ...` 同样报错。
- `python3 tools/run_fixtures.py` 跑出与旧 `scoop test` 等价的 pass/fail 集合与 checks 总数。
- `python3 tools/spec_fixtures.py check` / `tools/fixtures_matrix.py {check,stdlib}` / `tools/safepoint_baseline.py` / `tools/dependency_gate.py` / `tools/audit_*.py` 全部跑通，输出与旧 `scoop_tools` / `scoopc` 审计模块语义等价。
- `tools/run_fixture_scan.sh` / `tools/run_run_pass_gc_scan.sh` 用新入口跑通。
- `cargo test --all --all-targets` 通过（`scoopc` 单测在删除审计模块后不再依赖文件 grep；`p8_docs_cleanup` 测试在 §6 中清理后通过）。
- `.github/workflows/ci.yml` 内不再出现 `cargo run -p scoop_tools` / `scoop test` / `test-fixtures` 字样；CI 跑通。
- 全仓搜索 §7 步骤 5 列出的所有 token，在 `docs/archive/**` 与 `TODO*.md` / `PLAN*.md` 历史 “验证通过：…” 行之外无命中。
- §6 列出的文档全部更新。
