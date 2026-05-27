# Tools

该目录放置 Scoop 仓库的辅助工具和脚本。仓库级 fixture、spec 同步、审计与依赖检查都由这里的独立脚本驱动，不属于编译器或 driver CLI 的内置功能。

## Python Scripts

- `tools/run_fixtures.py`：fixture 套件驱动。默认扫描 `tests/fixtures/`，也可传入单个目录、case 目录或 `.scoop` 文件；通过公开编译器命令执行 parse / resolve / typecheck / infer / run-pass 等 fixture，并支持 `-j/--processes`、`--exit-on-failure`、`--gc-stress`、`--gc-move`、`--threads` 与 `-O/--opt-level`。
- `tools/spec_fixtures.py {sync,check}`：从 `SCOOP_FULL_SPEC.md` 中带 `// FIXTURE:` 的代码块生成或校验 `tests/fixtures/spec_doctest/`。`check --fix` 只写入内容变化的生成文件。
- `tools/fixtures_matrix.py {check,stdlib}`：生成 fixture 覆盖矩阵报告。`check` 核对 spec 章节与 doctest fixture 覆盖，`stdlib` 汇总 run-pass stdlib 领域覆盖。
- `tools/safepoint_baseline.py`：构建内置 safepoint workload，统计 LLVM IR 中 `statepoint` 与 `gc-live` roots 基线，并输出 Markdown 报告。
- `tools/dependency_gate.py`：基于 `cargo metadata --format-version 1` 检查 Scoop pipeline crate 的依赖方向和 source-boundary 残留。
- `tools/audit_spec_coverage.py`：审计 UMB fixture index、bucket 覆盖、spec coverage matrix 与负向诊断措辞守卫。
- `tools/audit_pipeline_gap.py`：审计 active pipeline gap inventory、LegacyOnly 残留、codegen scope-drift 基线与已关闭 blocker 守卫。
- `tools/audit_user_visible_failure_policy.py`：审计用户可见失败策略边界、frontend reject surface、upstream guard ledger、production `todo!` 与 internal bug sentinel 基线。

常用命令：

```bash
python3 tools/run_fixtures.py
python3 tools/spec_fixtures.py check
python3 tools/spec_fixtures.py sync
python3 tools/fixtures_matrix.py check
python3 tools/fixtures_matrix.py stdlib
python3 tools/safepoint_baseline.py
python3 tools/dependency_gate.py
python3 tools/audit_spec_coverage.py
python3 tools/audit_pipeline_gap.py
python3 tools/audit_user_visible_failure_policy.py
```

## Shell Helpers

- `tools/run_fixture_scan.sh`：逐个 fixture 调用 `python3 tools/run_fixtures.py <fixture>`，为每条 fixture 设置超时并把 pass / fail / timeout 汇总写入 `target/fixture-scan/`。
- `tools/run_run_pass_gc_scan.sh`：对 `tests/fixtures/run-pass/` 逐条运行 GC verify-roots、moving GC 与 stress 模式扫描，并把日志写入 `target/run-pass-gc-scan/`。
- `tools/gc_microbench.sh`：运行 GC microbench 的 `throughput` / `fragmentation` 场景，对比 baseline 与 Immix 后端；结果用于本地观测，不做阈值 gating。
