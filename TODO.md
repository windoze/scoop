# TODO（UnsupportedMainBody 收口计划，doc-and-test only）

> 生成时间：2026-05-18  
> 设计基线：[`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 格式参考：[`docs/archive/plans/TODO-closure-fix.md`](./docs/archive/plans/TODO-closure-fix.md)  
> 当前状态：U1-T02 已完成；下一项 U2-T01
> 执行原则：U0 必须最先完成；U1 → U2 → U3 严格串行；U4 与 U5 可在 U2/U3 稳定后并行，但 U5 的 negative fixture 必须引用 U4 已写明的 upstream gate；U6 必须最后完成。每个任务完成后必须回写“完成记录”。

## 全局约束

- [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) 是本轮唯一设计基线；[`PLAN.md`](./PLAN.md) 是本轮唯一执行计划基线。若要改变 bucket 编号、inventory schema、fixture 头部规范、P1-P6 范围或 doc-and-test only 边界，必须先更新这两个基线文档，再继续执行任务。
- 本计划是 **doc-and-test only**。允许产出 `.md`、`.scoop` fixture、`.stdout` / `.stderr` golden、`.csv` / `.json` inventory 数据、`#[cfg(test)]` audit 单元测试，以及 fixture runner / audit CLI 这类 test infrastructure。禁止在本计划内修 production LLVM codegen 的 `UnsupportedMainBody` 站点。
- 每一处 `LlvmEmitError::UnsupportedMainBody` 必须在 `audit/UMB_inventory.csv` 中拿到稳定 `UMB-NNNN` ID、唯一 bucket、`expected_class`、`spec_anchor`（helper invariant 除外）和 `upstream_gate`（B 类必须填实）。禁止 `bucket=TBD`、`expected_class=TBD` 合入。
- `expected_class` 只能是 `FrontendReject`、`InternalBugSentinel`、`RealImpl`，D 类 spec 缺口可在 bucket 文档中标 `D-pending`，但 CSV 中仍需明确当前治理路径。
- 不允许“兜底报错”继续扩散。执行本 TODO 时不得新增 `UnsupportedMainBody { kind: ... }` 站点，不得在 fixture 的 `EXPECT-ERROR` 文案中出现 `后端`、`backend`、`LLVM`、`codegen`、`UnsupportedMainBody`。
- 仓库内不得留下 failing fixture。`tests/fixtures/umb_fix/**` 的 fixture 要么 active 并通过，要么显式标记 `IGNORE-UNTIL-FIX:B-XX` / `ignore-until-fix:B-XX` 并由 runner 自动 skip / xfail。若现有 runner 不支持该标记，U5-T01 必须先补 test infrastructure。
- `crates/scoopc/src/audit/` 必须受 `#[cfg(test)]` 限定，不得进入 production codegen 链路。若改成独立 crate，必须是 workspace dev-dependency only。
- `audit/` 是仓库根文档和数据目录；`crates/scoopc/src/audit/` 是 Rust test module。两者不要混用。
- 每个任务完成后必须回写：改动范围、核心决策、验证结果、与 `PLAN.md` / `UnsupportedMainBody_FIX.md` 闭合的目标或验收项。

## 已知基线漂移风险（U0 已处理）

- U0-T01 已复算并同步更新 `PLAN.md` / `UnsupportedMainBody_FIX.md` 的设计基线：`LlvmEmitError::UnsupportedMainBody {` 在 `crates/scoopc/src/llvm/codegen/` 下 1,284 处、61 个文件、982 个不同 `kind:` 字面量标签、836 个单次出现字面量标签，`STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 为 638。
- U0 采用的 constructor 口径：匹配 `UnsupportedMainBody\s*\{`，且当前所有命中均为 `LlvmEmitError::UnsupportedMainBody {`；不排除 `#[cfg(test)]` 后内容；helper/shared path 计入总数。
- U0 采用的 `kind:` 标签口径：只统计 `kind: "..."` 字面量字段；当前有 1,247 个字面量字段命中，其余 37 个 constructor 使用动态或转发式 `kind`，仍计入 1,284 个 constructor 总数。
- 当前 `crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 的冻结测试断言 `total == 638`，已与 U0 基线一致。

## 固定定位清单

### 设计与计划文件

- `UnsupportedMainBody_FIX.md:1-13`：本轮终态目标与 doc-and-test only 范围。
- `UnsupportedMainBody_FIX.md:31-49`：三类治理路径与禁止“兜底报错”的硬约束。
- `UnsupportedMainBody_FIX.md:83-133`：P1 inventory 目标、产出位置与 CSV schema。
- `UnsupportedMainBody_FIX.md:136-210`：P2 bucket 分类和每个 bucket 文档必须包含的七段。
- `UnsupportedMainBody_FIX.md:214-266`：P3 spec 覆盖矩阵要求。
- `UnsupportedMainBody_FIX.md:269-337`：P4 修复策略模板与 A/B/C/D 类特殊规则。
- `UnsupportedMainBody_FIX.md:340-509`：P5 fixture 目录、`_index.csv` schema、fixture 头部规范、48 条 spec-driven fixture 与第 49 条 bucket-driven 对账要求。
- `UnsupportedMainBody_FIX.md:512-544`：P6 10 条 baseline test。
- `UnsupportedMainBody_FIX.md:548-571`：本计划阶段必须交付的文件总清单。
- `UnsupportedMainBody_FIX.md:575-599`：P7/P8 后续退场标准，本 TODO 不实现 production 修复，只给后续计划留可验证输入。
- `UnsupportedMainBody_FIX.md:638-649`：本计划自身退场条件。
- `PLAN.md:9-20`：工作原则，尤其 doc-and-test only、inventory 唯一真值、无临时 failing fixture、`crates/scoopc/src/audit/` 必须 `#[cfg(test)]`。
- `PLAN.md:21-72`：当前判断、基线表、既有资产和 36 个 bucket 候选概览。
- `PLAN.md:73-85`：设计目标。
- `PLAN.md:86-132`：inventory schema、bucket 编号、fixture 命名约束。
- `PLAN.md:134-151`：10 条 baseline test 名称与不变量。
- `PLAN.md:153-176`：U0-U6 顺序总览与依赖。
- `PLAN.md:178-408`：各任务详细计划，本文 TODO 按该段拆解。
- `PLAN.md:410-425`：本计划退场判据。
- `PLAN.md:427-460`：兼容性、迁移影响、风险与对策。

### 错误定义、现有审计与冻结基线

- `crates/scoopc/src/llvm/mod.rs:161-169`：`LlvmEmitError::UnsupportedMainBody { kind, at }` 定义，诊断码 `scoop::llvm::unsupported_main_body`。
- `crates/scoopc/src/pipeline_user_visible_failure_policy.rs:11-91`：`FAILURE_POLICY_AUDIT_FILES`，当前 failure policy 审计根集合。U6 的 audit 测试可以复用这些路径，也可以明确说明为何使用更宽的 `crates/scoopc/src/llvm/codegen/**`。
- `crates/scoopc/src/pipeline_user_visible_failure_policy.rs:93-103`：`FAILURE_KEYWORDS` 与 `FRONTEND_REJECT_FORBIDDEN_TERMS`。U5/U6 禁词测试必须复用或复制同一组禁词。
- `crates/scoopc/src/pipeline_user_visible_failure_policy.rs:141-311`：`STALE_UNSUPPORTED_MAIN_BODY_COUNTS`。当前 listed production source 总数为 638（见同文件 `assert_eq!(total, 638)`）。
- `crates/scoopc/src/pipeline_user_visible_failure_policy.rs:510-602`：当前已冻结的 frontend reject surfaces，包含上一轮 sealed interface gate。新增 UMB negative fixture 不应把错误文案写成 backend unsupported。
- `crates/scoopc/src/pipeline_user_visible_failure_policy.rs:604-657`：当前 stale unsupported marker 空表、post-upstream guards 和 internal bug sentinel hits。
- `crates/scoopc/src/pipeline_user_visible_failure_policy.rs:659-812`：现有 failure policy audit tests，可作为 U6 baseline test 的风格参考。
- `crates/scoopc/src/llvm/codegen_gap_inventory.rs:1-35`：现有 codegen-stage gap inventory 数据结构。
- `crates/scoopc/src/llvm/codegen_gap_inventory.rs:51-241`：当前 `CODEGEN_GAP_INVENTORY` 21 entries。U1/U2 必须与新的 `audit/UMB_inventory.csv` 做交叉引用，不要把这个旧表当全量 inventory。
- `docs/archive/designs/PIPELINE_GAPS.md`：历史 gap ledger，本轮只引用；P8 退场后才追加终态段落。

### 需要新增或填充的目标路径

- `audit/`：仓库根数据目录，目前不存在。U0 只创建空目录和 `.gitkeep`；U1 开始放真实产出。
- `audit/_baseline_files.txt`：U0 产出，记录当前命中 `UnsupportedMainBody` 的 codegen 文件清单和粗分组。
- `audit/_baseline_sampling.md`：U0 产出，记录 10 个抽样 entry 的 root cause 预演；U1 完成后可归档。
- `audit/UMB_inventory.csv`：U1 主表。
- `audit/UMB_inventory_schema.md`：U1 schema 文档。
- `audit/UMB_categories/_overview.md`：U2 bucket 总览。
- `audit/UMB_categories/B-01.md` 到 `audit/UMB_categories/B-36.md`：U2 bucket 文档。
- `audit/spec_coverage_matrix.md`：U3 spec 覆盖矩阵。
- `audit/strategies/B-01.md` 到 `audit/strategies/B-36.md`：U4 修复策略草案。
- `tests/fixtures/umb_fix/_index.csv`：U5 fixture index。
- `tests/fixtures/umb_fix/B-01-builder-invariant/` 到 `tests/fixtures/umb_fix/B-36-<slug>/`：U5 fixture 目录。
- `crates/scoopc/src/audit/umb_inventory.rs`：U6 测试 #1-#4；若 U1 就写入，必须确保 `#[cfg(test)]`。
- `crates/scoopc/src/audit/sentinel_tests.rs`：U6 测试 #10。
- `crates/scoopc/src/audit/spec_coverage.rs`：U6 测试 #5-#9。
- `UnsupportedMainBody_DONE.md`：U6-T02 退场占位文件。

### Rust module / bin 接入点

- `crates/scoopc/src/lib.rs:53-57`：当前已有 `#[cfg(test)] mod pipeline_gap_audit;` 与 `#[cfg(test)] mod pipeline_user_visible_failure_policy;`。新增 `audit` module 时应放在这里附近，并保持 `#[cfg(test)]`。
- `crates/scoopc/Cargo.toml:27-30`：当前只有 `[[bin]] name = "scoopc"`。若实现 `cargo run -p scoopc --bin umb-audit -- list/diff/stats`，需新增 `[[bin]]`，并明确它是 audit/test-only 工具；若选择独立 `crates/scoopc-audit/`，必须 workspace dev-only 并在完成记录写明原因。
- `crates/scoopc/src/bin/scoopc.rs`：现有 compiler bin，不要把 `umb-audit` 子命令混入 production `scoopc` CLI，除非文档明确说明它不参与 codegen 链路。

### Fixture runner 和 expectation 现状

- `crates/scoop/src/fixtures/mod.rs:1-37`：当前 fixture phase 路由说明。一级目录 `tests/fixtures/umb_fix/` 目前会被识别为 phase 但没有实现，U5-T01 必须决定是新增 phase、映射到现有 phase，还是让 audit tests 直接扫描 `_index.csv`。
- `crates/scoop/src/fixtures/mod.rs:158-260`：`plan_targets` 会递归收集 `.scoop` 文件，并跳过 multi/cone 特殊目录。U5-T01 若新增 `umb_fix` 目录，要避免它被“未知 phase”误跑失败。
- `crates/scoop/src/fixtures/mod.rs:430-559`：run_pass_cone 处理 `EXPECT: pass/fail` 的方式，可参考其 “fail 仍走内部 build 并断言稳定错误码” 逻辑。
- `crates/scoop/src/fixtures/mod.rs:3001-3033`：`assert_diagnostic_matches` 支持 `EXPECT-ERROR-CODE`、`EXPECT-ERROR-AT`、`EXPECT-ERROR`。
- `crates/scoop/src/fixtures/expectations.rs:1-28`：目前支持的 fixture 头部指令。当前没有 `IGNORE-UNTIL-FIX` / `ignore-until-fix` 支持。
- `crates/scoop/src/fixtures/expectations.rs:62-233`：`FixtureExpectation::from_source` 只扫描文件头前 32 行注释。U5 新增 `COVERS`、`BUCKETS`、`SPEC`、`REASON`、`IGNORE-UNTIL-FIX` 时要么扩展 parser，要么由 U6 audit scanner 单独解析。
- 当前全仓 `rg -n "IGNORE-UNTIL-FIX|ignore-until-fix"` 只在 `PLAN.md` 与 `UnsupportedMainBody_FIX.md` 中命中，runner 尚未支持该机制。

### Spec 与既有 fixture 位置

- `docs/spec/language_spec-part1.md` 到 `docs/spec/language_spec-part6.md`：U3 spec 覆盖矩阵唯一规范来源。
- `tests/fixtures/parse/**`：parse phase。
- `tests/fixtures/resolve/**`、`tests/fixtures/resolve_multi/**`、`tests/fixtures/resolve_cone/**`：resolve phase。
- `tests/fixtures/typecheck/**`、`tests/fixtures/typecheck_multi/**`、`tests/fixtures/typecheck_cone/**`、`tests/fixtures/typecheck_cone_archive/**`、`tests/fixtures/unsafe_nogc/**`：typecheck / unsafe gate phase。
- `tests/fixtures/infer/**`：infer phase。
- `tests/fixtures/comptime/**`：const/comptime phase。
- `tests/fixtures/build/**`：build phase，可用 `BUILD-LLVM-CONTAINS` / `BUILD-LLVM-NOT-CONTAINS`。
- `tests/fixtures/run-pass/**`、`tests/fixtures/codegen/**`、`tests/fixtures/runtime_gc/**`、`tests/fixtures/run_pass_cone/**`：run-pass / executable phase。
- `tests/fixtures/hir/**`、`tests/fixtures/mir/**`、`tests/fixtures/mir_lowered/**`、`tests/fixtures/effect_facts/**`、`tests/fixtures/effect_lowered/**`、`tests/fixtures/scoopir/**`：dump / golden phase。

### 当前 `UnsupportedMainBody` 粗粒度命中快照

以下为本 TODO 生成时 `rg -c 'UnsupportedMainBody \{' crates/scoopc/src/llvm/codegen` 的输出。U1 的正式 inventory 必须用脚本重建并以 CSV 为准；此表只用于减少初始定位搜索。

| 路径 | 当前命中数 |
|---|---:|
| `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs` | 139 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/payload.rs` | 2 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/main_entry.rs` | 9 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/runtime_error.rs` | 3 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/main_carrier.rs` | 5 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/states.rs` | 5 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/call_invoke.rs` | 4 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/wrapper.rs` | 1 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/composed_call.rs` | 2 |
| `crates/scoopc/src/llvm/codegen/layout.rs` | 19 |
| `crates/scoopc/src/llvm/codegen/enum_lowering.rs` | 29 |
| `crates/scoopc/src/llvm/codegen/ordinary_callee.rs` | 6 |
| `crates/scoopc/src/llvm/codegen/class_ctor.rs` | 31 |
| `crates/scoopc/src/llvm/codegen/stmt.rs` | 20 |
| `crates/scoopc/src/llvm/codegen/object_init.rs` | 13 |
| `crates/scoopc/src/llvm/codegen/gc.rs` | 70 |
| `crates/scoopc/src/llvm/codegen/ty.rs` | 15 |
| `crates/scoopc/src/llvm/codegen/intrinsics/sysroot.rs` | 16 |
| `crates/scoopc/src/llvm/codegen/intrinsics/named.rs` | 55 |
| `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs` | 58 |
| `crates/scoopc/src/llvm/codegen/main/coerce.rs` | 31 |
| `crates/scoopc/src/llvm/codegen/mir_body/const_pat.rs` | 38 |
| `crates/scoopc/src/llvm/codegen/intrinsics/atomic.rs` | 51 |
| `crates/scoopc/src/llvm/codegen/main/identity.rs` | 6 |
| `crates/scoopc/src/llvm/codegen/control_flow.rs` | 78 |
| `crates/scoopc/src/llvm/codegen/main/context.rs` | 4 |
| `crates/scoopc/src/llvm/codegen/mir_body/args.rs` | 15 |
| `crates/scoopc/src/llvm/codegen/intrinsics/sync.rs` | 53 |
| `crates/scoopc/src/llvm/codegen/main/numeric.rs` | 2 |
| `crates/scoopc/src/llvm/codegen/mir_body/transport.rs` | 10 |
| `crates/scoopc/src/llvm/codegen/main/function.rs` | 3 |
| `crates/scoopc/src/llvm/codegen/mir_body/string.rs` | 1 |
| `crates/scoopc/src/llvm/codegen/main/expr_value.rs` | 19 |
| `crates/scoopc/src/llvm/codegen/effect_outcome.rs` | 18 |
| `crates/scoopc/src/llvm/codegen/main/declare.rs` | 8 |
| `crates/scoopc/src/llvm/codegen/mir_body/value_args.rs` | 6 |
| `crates/scoopc/src/llvm/codegen/mir_body/aggregates.rs` | 31 |
| `crates/scoopc/src/llvm/codegen/main/boxing.rs` | 6 |
| `crates/scoopc/src/llvm/codegen/intrinsics/thread.rs` | 18 |
| `crates/scoopc/src/llvm/codegen/closure/mod.rs` | 24 |
| `crates/scoopc/src/llvm/codegen/mir_body/callable_lookup.rs` | 21 |
| `crates/scoopc/src/llvm/codegen/main/expr_op.rs` | 29 |
| `crates/scoopc/src/llvm/codegen/main/gc_locals.rs` | 11 |
| `crates/scoopc/src/llvm/codegen/main/literal.rs` | 4 |
| `crates/scoopc/src/llvm/codegen/mir_body/mod.rs` | 7 |
| `crates/scoopc/src/llvm/codegen/main/runtime_error.rs` | 4 |
| `crates/scoopc/src/llvm/codegen/mir_body/terminator.rs` | 19 |
| `crates/scoopc/src/llvm/codegen/main/alloca.rs` | 7 |
| `crates/scoopc/src/llvm/codegen/mir_body/call.rs` | 28 |
| `crates/scoopc/src/llvm/codegen/main/immut_value.rs` | 18 |
| `crates/scoopc/src/llvm/codegen/main/call.rs` | 12 |
| `crates/scoopc/src/llvm/codegen/expr.rs` | 11 |
| `crates/scoopc/src/llvm/codegen/mir_body/types.rs` | 4 |
| `crates/scoopc/src/llvm/codegen/main/globals.rs` | 11 |
| `crates/scoopc/src/llvm/codegen/mir_body/dispatch.rs` | 16 |
| `crates/scoopc/src/llvm/codegen/call/abi.rs` | 19 |
| `crates/scoopc/src/llvm/codegen/main/frame.rs` | 12 |
| `crates/scoopc/src/llvm/codegen/mir_body/cast.rs` | 28 |
| `crates/scoopc/src/llvm/codegen/mir_body/operand.rs` | 8 |
| `crates/scoopc/src/llvm/codegen/call/lowering.rs` | 41 |
| `crates/scoopc/src/llvm/codegen/mir_body/member.rs` | 50 |

### 当前 stale production count 快照

以下来自 `crates/scoopc/src/pipeline_user_visible_failure_policy.rs:146-311`。该表统计 production source text 中的 `UnsupportedMainBody` 字符串，不等同于上面的全 codegen 粗粒度表。

| 路径 | 冻结数 |
|---|---:|
| `crates/scoopc/src/llvm/codegen/mir_body/aggregates.rs` | 31 |
| `crates/scoopc/src/llvm/codegen/mir_body/args.rs` | 15 |
| `crates/scoopc/src/llvm/codegen/mir_body/call.rs` | 28 |
| `crates/scoopc/src/llvm/codegen/mir_body/callable_lookup.rs` | 21 |
| `crates/scoopc/src/llvm/codegen/mir_body/cast.rs` | 28 |
| `crates/scoopc/src/llvm/codegen/mir_body/const_pat.rs` | 38 |
| `crates/scoopc/src/llvm/codegen/mir_body/dispatch.rs` | 16 |
| `crates/scoopc/src/llvm/codegen/mir_body/member.rs` | 50 |
| `crates/scoopc/src/llvm/codegen/mir_body/mod.rs` | 6 |
| `crates/scoopc/src/llvm/codegen/mir_body/operand.rs` | 8 |
| `crates/scoopc/src/llvm/codegen/mir_body/string.rs` | 1 |
| `crates/scoopc/src/llvm/codegen/mir_body/terminator.rs` | 19 |
| `crates/scoopc/src/llvm/codegen/mir_body/transport.rs` | 10 |
| `crates/scoopc/src/llvm/codegen/mir_body/types.rs` | 4 |
| `crates/scoopc/src/llvm/codegen/mir_body/value_args.rs` | 6 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/call_invoke.rs` | 4 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/composed_call.rs` | 2 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/main_carrier.rs` | 5 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/main_entry.rs` | 9 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/payload.rs` | 2 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/runtime_error.rs` | 3 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/states.rs` | 5 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/wrapper.rs` | 1 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs` | 139 |
| `crates/scoopc/src/llvm/codegen/main/alloca.rs` | 7 |
| `crates/scoopc/src/llvm/codegen/main/boxing.rs` | 6 |
| `crates/scoopc/src/llvm/codegen/main/call.rs` | 12 |
| `crates/scoopc/src/llvm/codegen/main/coerce.rs` | 31 |
| `crates/scoopc/src/llvm/codegen/main/context.rs` | 4 |
| `crates/scoopc/src/llvm/codegen/main/declare.rs` | 8 |
| `crates/scoopc/src/llvm/codegen/main/expr_op.rs` | 29 |
| `crates/scoopc/src/llvm/codegen/main/expr_value.rs` | 19 |
| `crates/scoopc/src/llvm/codegen/main/frame.rs` | 12 |
| `crates/scoopc/src/llvm/codegen/main/function.rs` | 3 |
| `crates/scoopc/src/llvm/codegen/main/gc_locals.rs` | 11 |
| `crates/scoopc/src/llvm/codegen/main/globals.rs` | 11 |
| `crates/scoopc/src/llvm/codegen/main/identity.rs` | 6 |
| `crates/scoopc/src/llvm/codegen/main/immut_value.rs` | 18 |
| `crates/scoopc/src/llvm/codegen/main/literal.rs` | 4 |
| `crates/scoopc/src/llvm/codegen/main/numeric.rs` | 2 |
| `crates/scoopc/src/llvm/codegen/main/runtime_error.rs` | 4 |

## Bucket 稳定清单

除非 U1 inventory 完成后证明确需拆分或合并，否则 `B-01` 到 `B-36` 编号不得变更。

| Bucket | 名称 | 一级类 |
|---|---|---|
| B-01 | inkwell builder bookkeeping | A |
| B-02 | MIR local / member 类型推断不完整 | B |
| B-03 | MIR direct/closure/funptr 调用 ABI 漂移 | B |
| B-04 | MIR 函数签名 / 参数 / 返回类型缺失 | B |
| B-05 | MIR CFG / start block / goto target 异常 | B |
| B-06 | MIR struct/tuple/enum 字面量 schema 漂移 | B |
| B-07 | MIR pattern 子句 schema 漂移 | B |
| B-08 | MIR 成员存取 / 赋值合法性 | B |
| B-09 | Cross-TypeStore equivalence 不闭合 | B/C |
| B-10 | Effect-typed callable adapter / ABI routing | C |
| B-11 | Pure / plain statement 边界路由 | B |
| B-12 | Closure / lambda / capture 表达 | C |
| B-13 | 数组 / 复合 transport metadata | C |
| B-14 | Cast / TypeCheck (`as`/`as?`/`is`) | B/C |
| B-15 | When / 模式匹配用户面 | B |
| B-16 | 控制流 outside-of-context | B |
| B-17 | Coercion / 标量运算 | A/B |
| B-18 | 字面量与字符串 | B |
| B-19 | Top-level / object init / extern global | B |
| B-20 | Class ctor / property / 字段访问 | B |
| B-21 | Struct literal / 字段层 | B |
| B-22 | Enum 布局 / niche / Option | B |
| B-23 | Member access — 通用 | B |
| B-24 | Reflection / comptime intrinsic | C/D |
| B-25 | Platform / RTTI intrinsic | B/C |
| B-26 | atomic intrinsic 系列 | B |
| B-27 | sync intrinsic 系列 | B |
| B-28 | thread intrinsic 系列 | B |
| B-29 | GC intrinsic 系列 | B |
| B-30 | named / unsafe / FunPtr intrinsic | B |
| B-31 | 标量扩展方法 (Float/Int/Char/Bool/String) | B |
| B-32 | print / panic / sysroot 桥接 | B |
| B-33 | Extern global / FunPtr 顶层 | B |
| B-34 | RuntimeError / try-catch-finally | B |
| B-35 | unsafe / NoGC / 边界 | B/C |
| B-36 | 未定义/暂未支持的 spec surface | D |

## 顺序总览

```text
U0-T01 (摸底 + baseline 冻结)
  └─> U1-T01 (inventory 脚本 + CSV)
        └─> U1-T02 (schema + 索引子命令)
              └─> U2-T01 (bucket 分组 + 表头声明)
                    └─> U2-T02 (36 份 bucket md 主体)
                          └─> U3-T01 (spec 覆盖矩阵)
                                └─> U4-T01 (36 份策略草案)
                                      └─> U5-T01 (fixture 目录 + _index.csv)
                                            └─> U5-T02 (spec part 1-6 fixture 主体)
                                                  └─> U5-T03 (bucket-driven 直接对账 fixture)
                                                        └─> U6-T01 (10 条 baseline test)
                                                              └─> U6-T02 (退场标注 + 计划自检)
```

## U0：摸底 + baseline 冻结

### [DONE] U0-T01：现状摸底与基线冻结

- 参考：
  - [`PLAN.md`](./PLAN.md) §1、§6 U0
  - [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §0
  - 本文件“固定定位清单”和“已知基线漂移风险”
- 目标：
  - 在正式生成 inventory 前，确认当前源码中的计数口径、文件清单、冻结 stale count、现有 gap inventory 和 runner 能力。
  - 将后续 U1-U6 需要的初始事实落地到 `audit/_baseline_files.txt` 与 `audit/_baseline_sampling.md`，避免每个任务重复全仓搜索。
- 必须实现的内容：
  1. 创建仓库根 `audit/` 目录和 `audit/.gitkeep`。本任务不创建正式 inventory / bucket / strategy / fixture 主体。
  2. 复算 `PLAN.md:23-35` 的基线表，至少包含总 `UnsupportedMainBody` constructor 数、命中文件数、`kind:` 标签去重数、单次出现 `kind:` 标签数、`STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 总数、`CODEGEN_GAP_INVENTORY` entry 数。
  3. 明确统计口径：是否只统计 `crates/scoopc/src/llvm/codegen/**`，是否排除 `#[cfg(test)]` 之后内容，是否统计 helper path，是否统计 `LlvmEmitError::UnsupportedMainBody {` 与裸 `UnsupportedMainBody {` 两种形式。
  4. 把命中 `UnsupportedMainBody` 的 codegen 文件清单写入 `audit/_baseline_files.txt`，字段至少为 `path,umb_constructor_count,route_guess,notes`。`route_guess` 使用 `RawMirLlvm` / `EffectLoweredLlvm` / `Both` / `Helper`。
  5. 从四个一级类 A/B/C/D 各抽至少 2 个 entry，合计 10 个，写入 `audit/_baseline_sampling.md`。每个样本包含 `file:line`、`kind`、候选 bucket、root cause hypothesis、预期治理类、相关 spec 或 `N/A:helper-invariant`。
  6. 复核 `crates/scoopc/src/llvm/codegen_gap_inventory.rs` 的 21 entries，记录哪些 gap 已能映射到 B-XX，哪些需要 `notes` 标第二候选。
  7. 复核 fixture runner 是否支持 `IGNORE-UNTIL-FIX`。当前预期：`crates/scoop/src/fixtures/expectations.rs` 尚无该支持；若未支持，将 “runner 扩展” 明确列入 U5-T01。
  8. 处理已知漂移：若实测数字与 `PLAN.md` / `UnsupportedMainBody_FIX.md` 不一致，必须同步更新这两个文档的基线数字，或在 `audit/_baseline_sampling.md` 写明为什么统计口径不同且后续采用哪一种。
- 必须遵从的约束：
  - 不修改 production codegen。
  - 不手写正式 `audit/UMB_inventory.csv`，正式 CSV 必须由 U1 脚本产生。
  - 若 baseline 命令失败，记录命令、失败信息与是否阻塞 U1；不得顺手修无关问题。
- 建议验证命令：
  1. `cargo build`
  2. `cargo test --all --all-targets`
  3. `cargo run -p scoop -- test`
  4. `rg -n 'UnsupportedMainBody \{' crates/scoopc/src/llvm/codegen`
  5. `rg -c 'UnsupportedMainBody \{' crates/scoopc/src/llvm/codegen`
  6. `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
- 完成条件：
  - `audit/.gitkeep`、`audit/_baseline_files.txt`、`audit/_baseline_sampling.md` 落地。
  - 完成记录包含最终采用的 baseline 数字、统计口径、漂移处理结论、runner 支持状态、gap inventory 对账摘要。
- 依赖：无
- 完成记录：
  - 改动范围：新增 `audit/.gitkeep`、`audit/_baseline_files.txt`、`audit/_baseline_sampling.md`；同步更新 `PLAN.md`、`UnsupportedMainBody_FIX.md` 与本 TODO 的 U0 基线数字。
  - 核心决策：U0 constructor 采用 `crates/scoopc/src/llvm/codegen/**/*.rs` 内 `UnsupportedMainBody\s*\{` 全量口径，不排除 `#[cfg(test)]` 后内容，helper/shared path 计入总数；`kind:` 去重只统计 `kind: "..."` 字面量字段，动态/转发式 `kind` 由 U1 inventory 脚本继续精化。
  - 最终 baseline：1,284 个 `LlvmEmitError::UnsupportedMainBody {` constructor、61 个命中文件、1,247 个 `kind:` 字面量字段、982 个唯一 `kind:` 字面量、836 个单次出现 `kind:` 字面量、638 条 `STALE_UNSUPPORTED_MAIN_BODY_COUNTS`、21 个 `CODEGEN_GAP_INVENTORY` entry。
  - 漂移处理：确认源码已从旧设计基线 1,277/60/964/825/637 漂移；已更新 `PLAN.md` / `UnsupportedMainBody_FIX.md` 的阶段级基线与后续 U1/P6 expected count。
  - runner 支持状态：`crates/scoop/src/fixtures/expectations.rs` 与 Rust fixture runner 当前不支持 `IGNORE-UNTIL-FIX` / `ignore-until-fix`；U5-T01 必须先扩展 test infrastructure。
  - gap inventory 对账：21 个既有 `CODEGEN_GAP_INVENTORY` entry 均可映射到 B-01 到 B-36；存在第二候选的 entry 已记录在 `audit/_baseline_sampling.md` 的 notes 中。
  - 验证结果：`cargo build` 通过；`cargo test --all --all-targets` 通过（871 passed）；`cargo run -p scoop -- test` 通过（fixtures: ok, 1405 checks）；`rg -n 'UnsupportedMainBody \{' crates/scoopc/src/llvm/codegen` 与 `rg -c ...` 已运行；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 U0 的 `audit/` 目录、codegen 文件清单、10 个抽样 entry、漂移处理、runner 能力复核和 gap inventory 对账要求；后续 U1 以 U0 冻结的 1,284 constructor 基线启动正式 inventory。

## U1：P1 — Inventory 快照

### [DONE] U1-T01：inventory 脚本 + CSV 主表

- 参考：
  - [`PLAN.md`](./PLAN.md) §3.1、§6 U1-T01
  - [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §2
  - U0 的 `audit/_baseline_files.txt` 与 `audit/_baseline_sampling.md`
- 目标：
  - 用可重复脚本从源码重建 `audit/UMB_inventory.csv`，让每一处 `UnsupportedMainBody` 有稳定 ID、bucket、class、spec 锚和 upstream gate。
- 必须实现的内容：
  1. 新增 `crates/scoopc/src/audit/umb_inventory.rs`，并确保整个 audit module 只在 `#[cfg(test)]` 下编译。
  2. 实现源码扫描逻辑，按 U0 决定的统计口径查找每一处 `UnsupportedMainBody` constructor，并提取 `file`、1-based `line`、`kind` 字面量、route、surface。
  3. 按 `file+line` 稳定排序生成 `UMB-NNNN`。编号一旦落入 CSV，后续只允许在源码行实质变动时按脚本重排并记录原因。
  4. 生成 `audit/UMB_inventory.csv`，字段必须严格为 `id,file,line,kind,route,surface,bucket,expected_class,spec_anchor,upstream_gate,existing_fixture,notes`。
  5. 每条 entry 的 `bucket` 必须属于 B-01 到 B-36；若某 entry 无法归类，不允许写 `TBD` 后合入，必须先补 bucket 决策。
  6. 每条 entry 的 `expected_class` 必须是 `FrontendReject` / `InternalBugSentinel` / `RealImpl`。D 类缺口用 `notes` 或 bucket md 标 `D-pending`，CSV 仍给出当前治理路径。
  7. 每条非 helper invariant entry 的 `spec_anchor` 必须非空；helper invariant 使用 `N/A:helper-invariant`。
  8. B 类 entry 的 `upstream_gate` 可以在 U1 初版先写候选，但 U4-T01 完成前必须全部填实；若 U1 已能确认，直接填真实 gate。
  9. 脚本必须验证 CSV 行数等于源码扫描 entry 数，每个 `kind` 在 CSV 中出现次数等于源码中出现次数。
  10. 与 `crates/scoopc/src/llvm/codegen_gap_inventory.rs` 做交叉引用：旧 gap entry 能映射的写入 `notes` 或 `existing_fixture`，不能映射的写入 U1 完成记录。
- 必须遵从的约束：
  - 不用手维护 CSV 行数；CSV 由脚本生成或校验。
  - 不新增 production `UnsupportedMainBody`。
  - 不为了让 CSV 好看而改源码行号或移动 codegen。
- 验证：
  1. `cargo test -p scoopc audit::umb_inventory -- --nocapture`
  2. `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  3. 运行 U1 脚本的 CSV diff 检查，确认源码和 `audit/UMB_inventory.csv` 同步。
- 完成条件：
  - `audit/UMB_inventory.csv` 落地，行数等于 U0 冻结的正式 entry 数。
  - `bucket=TBD`、`expected_class=TBD` 不存在。
  - 每个 `kind` 字面量源码出现次数与 CSV 出现次数一致。
- 依赖：U0-T01
- 完成记录：
  - 改动范围：新增 `crates/scoopc/src/audit/mod.rs`、`crates/scoopc/src/audit/umb_inventory.rs` 与正式生成的 `audit/UMB_inventory.csv`；在 `crates/scoopc/src/lib.rs` 中以 `#[cfg(test)] mod audit;` 接入测试专用 audit module。
  - 核心决策：U1 scanner 递归扫描 `crates/scoopc/src/llvm/codegen/**/*.rs`，按 `file + line + column` 稳定排序生成 `UMB-NNNN`；CSV 表头严格为 `id,file,line,kind,route,surface,bucket,expected_class,spec_anchor,upstream_gate,existing_fixture,notes`；动态或转发式 `kind` 以 `DYNAMIC:<expr>` 记录，不伪造字面量。
  - 分类结果：CSV 共 1,284 条 entry，覆盖 B-01 到 B-36 且无空 bucket；每条 entry 都有非 `TBD` 的 `bucket`、`expected_class`、`spec_anchor` 和 `upstream_gate`；helper invariant 使用 `spec_anchor=N/A:helper-invariant` 且 `expected_class=InternalBugSentinel`。
  - kind 对账：正式 constructor-scoped inventory 中有 1,241 条 literal `kind` entry 和 43 条 dynamic/forwarded `kind` entry；测试按扫描结果对账每个 literal `kind` 在源码与 CSV 中的出现次数。
  - gap inventory 对账：21 个既有 `CODEGEN_GAP_INVENTORY` / `PIPELINE_GAPS` entry 均通过 `notes` 中的 `legacy_gap=...` 映射到 B-01 到 B-36 的主 bucket；未发现无法映射项。
  - 验证结果：`SCOOP_WRITE_UMB_INVENTORY=1 cargo test -p scoopc audit::umb_inventory::umb_inventory_csv_in_sync -- --nocapture` 通过并生成 CSV；`cargo test -p scoopc audit::umb_inventory -- --nocapture` 通过（3 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo clippy --all-targets -- -D warnings` 通过；`audit/UMB_inventory.csv` 未发现 `TBD`，且包含 1,284 条 `UMB-` 数据行。
  - 闭合目标：满足 U1-T01 的可重复源码扫描、稳定 ID、CSV 主表落地、bucket/class/spec/gate 填实、kind 计数对账和旧 gap inventory 交叉引用要求；后续 U1-T02 可在此模块逻辑上实现 `umb-audit list/diff/stats` 与 schema 文档。

### [DONE] U1-T02：inventory schema 文档 + 索引子命令

- 参考：
  - [`PLAN.md`](./PLAN.md) §3.1、§6 U1-T02
  - [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §2.3-§2.5
  - `crates/scoopc/Cargo.toml:27-30`
- 目标：
  - 让 inventory 数据格式可人工 review、可机器 diff，并提供 `list` / `diff` / `stats` 三个入口。
- 必须实现的内容：
  1. 写 `audit/UMB_inventory_schema.md`，逐字段定义取值域、合法性规则、排序规则、CSV escaping 规则和对账规则。
  2. 在 schema 文档中复制 B-01 到 B-36 的合法 bucket 表，说明拆分 / 合并流程必须同步更新 `PLAN.md` 与 `UnsupportedMainBody_FIX.md`。
  3. 实现 `umb-audit list`：按 bucket / file / class 列 entry，至少支持 `--bucket B-XX`。
  4. 实现 `umb-audit diff`：重扫源码并与 `audit/UMB_inventory.csv` 比较；输出新增、删除、line drift、kind drift、field drift。
  5. 实现 `umb-audit stats`：输出每 bucket entry 数、每 class entry 数、每 file entry 数、无 spec anchor 数、无 upstream gate 数。
  6. 选择 CLI 落地方式：新增 `cargo run -p scoopc --bin umb-audit -- ...`，或建立 dev-only `crates/scoopc-audit/`。完成记录必须写明选择理由。
  7. 如果新增 `[[bin]]`，不得破坏现有 `scoopc` bin；如果需要 `required-features`，需记录 LLVM 依赖要求。
- 必须遵从的约束：
  - `umb-audit` 不参与 production codegen；如果无法严格隔离，必须改为 test-only module + cargo test 入口。
  - `diff` 子命令必须是 CI 可用的非交互命令。
- 验证：
  1. `cargo run -p scoopc --bin umb-audit -- list --bucket B-02`
  2. `cargo run -p scoopc --bin umb-audit -- diff`
  3. `cargo run -p scoopc --bin umb-audit -- stats`
  4. `cargo test -p scoopc audit::umb_inventory -- --nocapture`
- 完成条件：
  - `audit/UMB_inventory_schema.md` 落地且字段完整。
  - 三个子命令在当前 checkout 跑通，`diff` 无漂移。
  - U6 baseline test #1 可直接复用此逻辑。
- 依赖：U1-T01
- 完成记录：
  - 改动范围：新增 `audit/UMB_inventory_schema.md`；新增 `crates/scoopc/src/bin/umb-audit.rs` 与 `Cargo.toml` bin target；将 `crates/scoopc/src/audit/umb_inventory.rs` 的扫描、校验、渲染入口开放给审计 bin 复用。
  - 核心决策：采用 `cargo run -p scoopc --bin umb-audit -- ...` 作为 U1 索引入口，不新建 dev-only crate；理由是可直接复用 U1 已冻结的 scanner/classification/render 逻辑，避免第二套 inventory 规则漂移，且该 bin 只读源码和 audit CSV，不接入 production `scoopc` 编译入口或 LLVM codegen 链路。
  - schema 文档：`audit/UMB_inventory_schema.md` 已逐字段定义取值域、合法性规则、排序规则、CSV escaping 规则和对账规则，并复制 B-01 到 B-36 合法 bucket 表；bucket 拆分/合并流程明确要求同步更新 `PLAN.md` 与 `UnsupportedMainBody_FIX.md`。
  - CLI 能力：`umb-audit list` 支持 `--bucket B-XX`、`--file PATH`、`--class CLASS`；`diff` 会重扫源码并报告新增、删除、line drift、kind drift、field drift，当前 checkout 无漂移；`stats` 输出每 bucket、每 class、每 file entry 数，以及缺失 `spec_anchor` / `upstream_gate` 数。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-02` 通过（6 entries）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（1,284 entries in sync）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（missing spec/gate 均为 0）；`cargo test -p scoopc audit::umb_inventory -- --nocapture` 通过（3 passed）；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 U1-T02 的 schema 文档、机器 diff/list/stats 入口和 U6 baseline test #1 复用要求；后续 U2-T01 可直接使用 `umb-audit stats` 生成 bucket 总览数字。

## U2：P2 — 成因分析与 Bucket 文档

### [TODO] U2-T01：bucket 分组确认 + md 表头声明

- 参考：
  - [`PLAN.md`](./PLAN.md) §1.4、§6 U2-T01
  - [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §3.2-§3.4
  - `audit/UMB_inventory.csv`
- 目标：
  - 把 U1 inventory 汇总到 bucket 维度，固定 36 份 bucket 文档骨架和总览数字。
- 必须实现的内容：
  1. 新建 `audit/UMB_categories/`。
  2. 生成 `audit/UMB_categories/_overview.md`，表格列为 `bucket / 名称 / 一级类 / entry 数 / expected_class 分布 / 主要 kind 标签前 5 条 / 主要文件前 5 个 / 备注`。
  3. 创建 `audit/UMB_categories/B-01.md` 到 `B-36.md`，每份包含七段标题：`Symptom`、`Root Cause Hypothesis`、`Spec Linkage`、`Expected Post-Fix Class`、`Fix Strategy Outline`、`Fixture Set Pointer`、`Open Questions`。
  4. 每份 bucket md 顶部必须声明 `本 bucket entry 数：N`、`inventory 来源：audit/UMB_inventory.csv`、`生成时间`、`负责人/状态`。
  5. 任一 bucket entry 数为 0 时，不得静默保留空 bucket。必须决定保留理由、合并或拆分，并同步更新 `PLAN.md` §1.4 与 `UnsupportedMainBody_FIX.md` §3.2。
  6. `_overview.md` 必须给出一个完整 B-01 样例，供多人并行写 U2-T02 / U4-T01 时保持格式。
- 必须遵从的约束：
  - 本任务只创建骨架和总览，不编造 root cause 主体。
  - 数字以 `audit/UMB_inventory.csv` 为唯一来源。
- 验证：
  1. `cargo run -p scoopc --bin umb-audit -- stats`
  2. `cargo test -p scoopc audit::umb_inventory -- --nocapture`
  3. 手工确认 36 份 `audit/UMB_categories/B-XX.md` 全部存在。
- 完成条件：
  - `_overview.md` 每个 bucket entry 数与 CSV 完全一致。
  - 36 份骨架 md 全部存在且标题一致。
- 依赖：U1-T02
- 完成记录：待填写

### [TODO] U2-T02：36 份 bucket md 主体

- 参考：
  - [`PLAN.md`](./PLAN.md) §6 U2-T02
  - [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §3.3
  - `audit/UMB_categories/_overview.md`
- 目标：
  - 为每个 bucket 写完整成因分析，让后续 U4/U5/P7 不需要再从零读 codegen。
- 必须实现的内容：
  1. `Symptom`：从 CSV 抽出该 bucket 下所有 `(id, file, line, kind, route, surface, expected_class)`，直接贴表。不得只写摘要。
  2. `Symptom`：附 3 个最具代表性的源码片段，每个片段含上下文 ±10 行、对应 `UMB-NNNN`、为何代表该 bucket。
  3. `Root Cause Hypothesis`：说明上游哪个阶段的什么不变量缺失，例如 typecheck gate、HIR lowering contract、MIR strict verifier、materialize contract、codegen helper invariant。
  4. `Spec Linkage`：列出所有相关 `docs/spec/language_spec-partN.md#...` 锚；helper invariant 使用 `N/A:helper-invariant` 并说明原因。
  5. `Expected Post-Fix Class`：分表列 `FrontendReject` / `InternalBugSentinel` / `RealImpl` / `D-pending` 数量，合计必须等于本 bucket entry 数。
  6. `Fix Strategy Outline`：一句话高层策略；详细策略留给 U4-T01。
  7. `Fixture Set Pointer`：预留 `tests/fixtures/umb_fix/B-XX-<slug>/`，U5 完成后回填具体 fixture。
  8. `Open Questions`：记录 spec 沉默、bucket 边界争议、upstream gate 尚未存在等问题。
  9. 对跨类 bucket（B-09、B-14、B-17、B-24、B-25、B-35）必须按 entry 级别拆分 class，不能只给 bucket 级 class。
- 必须遵从的约束：
  - 不修改 production codegen。
  - 不把 `UnsupportedMainBody` 文案带进用户可见 fixture 错误消息。
  - 不把无法判断的内容留成 `TBD`；如果确有未知，写入 `Open Questions` 并给出阻塞谁。
- 验证：
  1. `cargo run -p scoopc --bin umb-audit -- stats`
  2. `cargo test -p scoopc audit::umb_inventory -- --nocapture`
  3. 后续 U6 baseline test #2-#4 草稿可对 36 份 md 做数字对账。
- 完成条件：
  - 36 份 bucket md 全部成文。
  - 每个 bucket md 的 class 分布数字之和等于 CSV 中该 bucket entry 数。
  - 每条 inventory entry 能在对应 bucket md 的 symptom 表中找到。
- 依赖：U2-T01
- 完成记录：待填写

## U3：P3 — Spec 覆盖矩阵

### [TODO] U3-T01：编写 `audit/spec_coverage_matrix.md`

- 参考：
  - [`PLAN.md`](./PLAN.md) §6 U3-T01
  - [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §4
  - `docs/spec/language_spec-part1.md` 到 `docs/spec/language_spec-part6.md`
  - `audit/UMB_inventory.csv` 与 `audit/UMB_categories/B-XX.md`
- 目标：
  - 建立 spec section、现有 fixture、新增 fixture、bucket、inventory entry 的双向回链。
- 必须实现的内容：
  1. 新建 `audit/spec_coverage_matrix.md`，按 part1 到 part6 编排。
  2. 每个 spec section 一行，列为 `Spec 锚 / 语法特性 / 现有正例 / 现有负例 / 新增正例 / 新增负例 / 关联 buckets / 关联 UMB ids / 备注`。
  3. 扫现有 fixture 目录，尽量填充“现有正例 / 现有负例”。优先使用固定定位清单里的 phase 目录，不要重新猜 runner 路由。
  4. 对 U5 将新增但尚未存在的 fixture，在“新增正例 / 新增负例”中写计划路径占位，例如 `tests/fixtures/umb_fix/B-15-when-pattern/pos_when_enum_exhaustive.scoop (planned)`。
  5. `audit/UMB_inventory.csv` 中每条非 helper invariant entry 的 `spec_anchor` 必须能在矩阵中找到对应行。
  6. spec part4 中 async / generator / yield 等未定义区域必须写 `INTENTIONALLY-EMPTY: <spec 原句引用>`，并关联 B-36 或相应 D 类 bucket。
  7. 若 spec 沉默但 codegen 有 entry，矩阵备注写 `BlockedOnSpec`，U5 写 frontend-reject negative fixture，U4 策略归 D 类或 B/D 过渡。
  8. 为 U6 test #8 准备可机器扫描的 fixture 引用格式，不要只写自然语言。
- 必须遵从的约束：
  - 不修改 spec 文档本身。
  - 不为了覆盖矩阵而新增不可运行 fixture；新增 fixture 留给 U5。
- 验证：
  1. `cargo run -p scoopc --bin umb-audit -- stats`
  2. 手工检查矩阵中每个 bucket 链接都存在。
  3. 后续 U6 baseline test #8 草稿能解析矩阵中的 fixture 引用。
- 完成条件：
  - `audit/spec_coverage_matrix.md` 落地。
  - spec part1-6 每个 section 有现有/新增 fixture 或明确 `INTENTIONALLY-EMPTY`。
  - 无 inventory entry 找不到 spec 锚（helper invariant 除外）。
- 依赖：U2-T02
- 完成记录：待填写

## U4：P4 — 修复策略草案

### [TODO] U4-T01：编写 36 份 `audit/strategies/B-XX.md`

- 参考：
  - [`PLAN.md`](./PLAN.md) §6 U4-T01
  - [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §5
  - `audit/UMB_categories/B-XX.md`
  - `audit/spec_coverage_matrix.md`
- 目标：
  - 为 P7 production 修复提前定义每个 bucket 的 upstream contract、落地路径和验证锚，但本任务不实现 production 修复。
- 必须实现的内容：
  1. 新建 `audit/strategies/`。
  2. 创建 `audit/strategies/B-01.md` 到 `B-36.md`。
  3. 每份策略包含三段固定标题：`上游契约`、`落地路径`、`验证锚`。
  4. `上游契约` 明确责任阶段：`typecheck`、`hir`、`mir`、`mir.materialize`、`strict verifier`、`codegen helper` 之一或组合。B 类每条 entry 必须唯一 gate。
  5. `落地路径` 按 A/B/C/D 分类写清：A 类抽 helper / `unreachable!`，B 类 explicit reject / verifier baseline / codegen sentinel，C 类 fixture 驱动 real impl，D 类先 spec follow-up 或 frontend reject。
  6. `验证锚` 引用 U5 fixture 计划路径、U6 baseline test、退场计数器、需要下调的 inventory / stale count。
  7. A 类 bucket（B-01、B-17 部分、B-35 部分）必须设计统一 helper。建议命名从 `UnsupportedMainBody_FIX.md` §5.2 取：`MainCodegen::expect_insert_block`、`expect_parent_function`、`expect_entry_block`、`expect_basic_value`。
  8. B 类 entry 的 `upstream_gate` 必须同步回填到 `audit/UMB_inventory.csv`，不允许 U4 完成后仍有 B 类 gate 为空或 `TBD`。
  9. C 类 bucket 必须列出最小 happy-path fixture 名称，并标明在 P7 修复前需 `IGNORE-UNTIL-FIX:B-XX`。
  10. D 类 bucket 必须列出 spec follow-up 阻塞点，并说明当前 negative fixture 应锁定的 frontend reject 诊断。
- 必须遵从的约束：
  - 策略文档不是 production 修改许可。本任务不得删除或改写 codegen `UnsupportedMainBody`。
  - 不把同一 entry 分配给多个 primary gate；第二候选只写 `notes`。
- 验证：
  1. `cargo run -p scoopc --bin umb-audit -- diff`
  2. `cargo run -p scoopc --bin umb-audit -- stats`
  3. 检查 36 份 strategy md 均存在且三段不空白。
- 完成条件：
  - 36 份策略 md 全部成文。
  - 每条 B 类 entry 的 `upstream_gate` 在 CSV 中已填实。
  - 每份策略有明确 fixture / baseline test 验证锚。
- 依赖：U3-T01
- 完成记录：待填写

## U5：P5 — Fixture 集合

### [TODO] U5-T01：fixture 目录骨架 + `_index.csv` + runner 支持确认

- 参考：
  - [`PLAN.md`](./PLAN.md) §3.3、§6 U5-T01、§8、§9
  - [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §6.1、§6.3、§6.5
  - `crates/scoop/src/fixtures/mod.rs:1-37`
  - `crates/scoop/src/fixtures/expectations.rs:1-28`
- 目标：
  - 建立 `tests/fixtures/umb_fix/**` 的目录、索引和 skip / xfail 机制，确保后续 fixture 不会以未知 phase 或临时 failing 形式污染仓库。
- 必须实现的内容：
  1. 创建 `tests/fixtures/umb_fix/`。
  2. 创建 `tests/fixtures/umb_fix/_index.csv`，表头严格为 `fixture_path,bucket,kind,spec_anchor,umb_ids,status,notes`。
  3. 创建 36 个 bucket 目录，slug 建议：`B-01-builder-invariant`、`B-02-mir-local-type`、`B-03-call-abi`、`B-04-function-signature`、`B-05-mir-cfg`、`B-06-literal-schema`、`B-07-pattern-schema`、`B-08-member-store`、`B-09-type-equivalence`、`B-10-effect-callable-adapter`、`B-11-pure-boundary`、`B-12-closure-capture`、`B-13-composite-transport`、`B-14-cast-typecheck`、`B-15-when-pattern`、`B-16-control-flow-context`、`B-17-coercion-scalar`、`B-18-literals-strings`、`B-19-top-level-object-extern`、`B-20-class-property-field`、`B-21-struct-fields`、`B-22-enum-niche-option`、`B-23-member-access`、`B-24-reflection-comptime`、`B-25-platform-rtti`、`B-26-atomic-intrinsics`、`B-27-sync-intrinsics`、`B-28-thread-intrinsics`、`B-29-gc-intrinsics`、`B-30-named-unsafe-funptr`、`B-31-scalar-methods`、`B-32-print-panic-sysroot`、`B-33-extern-global-funptr`、`B-34-runtime-error-try`、`B-35-unsafe-nogc-boundary`、`B-36-spec-uncovered`。
  4. 每个目录创建 `_README.md`，链接对应 `audit/UMB_categories/B-XX.md`、`audit/strategies/B-XX.md`、`audit/spec_coverage_matrix.md` 行和 `_index.csv` 条目说明。
  5. 确认现有 runner 对 `tests/fixtures/umb_fix/**` 的行为。当前预期没有专门 phase，也没有 `IGNORE-UNTIL-FIX` parser 支持。
  6. 如果 runner 不支持，新增 test infrastructure：让 `cargo run -p scoop -- test tests/fixtures/umb_fix/` 能识别 `IGNORE-UNTIL-FIX:B-XX` 并 skip / xfail，且 unknown phase 不失败。
  7. 若选择不让 fixture runner 直接跑 `umb_fix`，必须让 U6 audit tests 完整验证 `_index.csv`、文件存在性、active/ignore 状态，并确保 `cargo run -p scoop -- test tests/fixtures/umb_fix/` 按 PLAN 退场要求可通过。
  8. 在 `_index.csv` 先填 skeleton / planned 条目时，`status` 只能是 `active` 或 `ignore-until-fix:B-XX`。
- 必须遵从的约束：
  - 不创建会失败但未标 ignore 的 fixture。
  - 不让 `umb_fix` 目录被现有 runner 作为未知 phase 直接失败。
  - 任何 runner 改动仅限 test infrastructure，不得改变 production compiler semantics。
- 验证：
  1. `cargo run -p scoop -- test tests/fixtures/umb_fix/`
  2. `cargo test -p scoop -- fixtures -- --nocapture`
  3. 后续 U6 test #5 草稿能扫描 `_index.csv` 与目录。
- 完成条件：
  - 36 个目录和 `_README.md` 全部存在。
  - `_index.csv` 表头与 schema 一致。
  - `IGNORE-UNTIL-FIX` 支持状态明确；若需要 runner 改动，已落地并验证。
- 依赖：U4-T01 可先行；U3-T01 必须已完成
- 完成记录：待填写

### [TODO] U5-T02：spec part1-6 fixture 主体

- 参考：
  - [`PLAN.md`](./PLAN.md) §6 U5-T02
  - [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §6.3-§6.5
  - `audit/spec_coverage_matrix.md`
  - `audit/strategies/B-XX.md`
- 目标：
  - 按 spec part1-6 落地 48 组 fixture，补齐语言特性的正/负例覆盖。
- 必须实现的内容：
  1. Part 1：整型 / 浮点 / 字符 / 字符串 / 插值 / `\u{...}`；包声明 + cone 边界；顶层声明全覆盖。
  2. Part 2：标量值类型 + 装箱；引用类型；值类型 nominal；Option niche；泛型；类型别名；function type；GC-free；`with` copy-update。
  3. Part 3：运算符优先级；函数定义；lambda；extension fun；属性；控制流；when；`as` / `as?` / `is`；数组；range；struct literal / `do`；operator overloading；class literal；类型推断。
  4. Part 4：effect 声明；effect row；handler；handler finally；try/catch/finally；required effect 推断；四种合法 main；async / generator / yield 反样本。
  5. Part 5：`const fun` / `const val`；`comptime if/for`；reflection intrinsic；splice field；Platform；RTTI；annotation；`@Intrinsic`。
  6. Part 6：unsafe block；safe region；`@NoGC`；raw pointer；FFI；GC.handle*。
  7. 每组 fixture 至少 1 个 positive 和 1 个 negative，除非 spec 明确该形态总是合法；例外必须在矩阵备注和 fixture index 中写明。
  8. 每个 fixture 文件头必须包含 `EXPECT`、`SPEC`、`COVERS`、`BUCKETS`，negative 还必须包含 `EXPECT-ERROR-CODE`、`EXPECT-ERROR-AT`、`EXPECT-ERROR`、`REASON`。
  9. 每新增一个 fixture，必须同步更新 `tests/fixtures/umb_fix/_index.csv` 和 `audit/spec_coverage_matrix.md`。
  10. C 类 happy-path fixture 在 P7 修复前必须标 `IGNORE-UNTIL-FIX:B-XX`，index 中 `status=ignore-until-fix:B-XX`。
- 必须遵从的约束：
  - 不使用 sysroot 之外尚未定义的库 API。
  - fixture 间不要互相 import，保持单文件可读。
  - negative fixture 文案不得包含 forbidden terms。
  - 不新增 production `UnsupportedMainBody` 站点。
- 验证：
  1. `cargo run -p scoop -- test tests/fixtures/umb_fix/`
  2. `cargo test -p scoopc audit::spec_coverage -- --nocapture`（若 U6 草稿已存在）
  3. 对涉及 build/run 的 fixture，按 phase 补充 `cargo run -p scoop -- test tests/fixtures/build/` 或 `tests/fixtures/run-pass/` 定向命令。
- 完成条件：
  - 48 组 spec fixture 全部落地，或有明确 spec-always-legal / intentionally-empty 记录。
  - `_index.csv` 与 spec 覆盖矩阵同步。
  - 所有 active fixture 通过；ignore fixture 被 runner 正确 skip / xfail。
- 依赖：U5-T01；U4-T01 的 gate 说明应已可引用
- 完成记录：待填写

### [TODO] U5-T03：bucket-driven 直接对账 fixture

- 参考：
  - [`PLAN.md`](./PLAN.md) §6 U5-T03
  - [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §6.2、§6.4 第 49 条
  - `audit/UMB_inventory.csv`
  - `audit/UMB_categories/B-XX.md`
  - `audit/strategies/B-XX.md`
- 目标：
  - 确保每个 bucket 至少有专门 fixture，每个 inventory id 都能被 fixture 或 sentinel test 覆盖。
- 必须实现的内容：
  1. 对 B-01 到 B-36，每个 bucket 至少新增或指定 1 条专属 fixture。
  2. 每条 fixture 顶部 `// COVERS:` 列出该 fixture 驱动消除的 `UMB-NNNN` 列表。
  3. `_index.csv` 的 `umb_ids` 字段必须与 fixture 头部 `COVERS` 完全一致。
  4. 一条 fixture 可以覆盖多个 entry；优先多对一以控制文件数量。U5-T03 完成记录必须统计平均覆盖率。
  5. A 类 helper invariant 无法从用户 fixture 直接构造时，需在 `_index.csv` 或 bucket README 中指向 U6 `sentinel_tests.rs`，并在 `status` / `notes` 说明。
  6. B 类 negative fixture 必须验证 U4 指定的 upstream gate，错误码和位置稳定。
  7. C 类 positive fixture 必须标 `IGNORE-UNTIL-FIX:B-XX`，直到 P7 实现后再转 active。
  8. D 类 fixture 标 `D-pending` 或 frontend reject，且矩阵中有 `INTENTIONALLY-EMPTY` 或 `BlockedOnSpec` 说明。
  9. 每新增 fixture 必须回填 `audit/UMB_categories/B-XX.md` 的 `Fixture Set Pointer` 和 `audit/strategies/B-XX.md` 的 `验证锚`。
- 必须遵从的约束：
  - 不为了覆盖 UMB id 构造非法但无 stable diagnostic 的 fixture。
  - 不把 `UnsupportedMainBody` 暴露给 fixture 期望输出。
- 验证：
  1. `cargo run -p scoop -- test tests/fixtures/umb_fix/`
  2. `cargo test -p scoopc audit::spec_coverage -- --nocapture`（若 U6 草稿已存在）
  3. `cargo run -p scoopc --bin umb-audit -- stats`
- 完成条件：
  - 每个 bucket 至少一条专属 fixture 或 sentinel test 指针。
  - 每个 inventory id 至少出现在一条 `COVERS` 或 sentinel coverage 记录中；D 类例外必须显式标注。
  - `_index.csv`、bucket md、strategy md、spec matrix 互相闭环。
- 依赖：U5-T02；U4-T01
- 完成记录：待填写

## U6：P6 — Baseline 测试与退场

### [TODO] U6-T01：10 条 baseline test 落地

- 参考：
  - [`PLAN.md`](./PLAN.md) §4、§6 U6-T01
  - [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §7
  - `crates/scoopc/src/pipeline_user_visible_failure_policy.rs:659-812`
- 目标：
  - 用 Rust baseline tests 把 U1-U5 的所有数据、文档和 fixture 互相锁住。
- 必须实现的内容：
  1. `crates/scoopc/src/audit/umb_inventory.rs` 实现 `umb_inventory_csv_in_sync`：CSV 与源码 grep 重建结果完全一致，行数等于 U0/U1 冻结值。
  2. 同文件实现 `umb_inventory_buckets_total`：每个 bucket entry 数等于 bucket md 表头声明，总和等于冻结值。
  3. 同文件实现 `umb_inventory_each_entry_has_spec_anchor_or_helper_marker`。
  4. 同文件实现 `umb_inventory_class_distribution`：三类 entry 数与 bucket md `Expected Post-Fix Class` 对账。
  5. `crates/scoopc/src/audit/spec_coverage.rs` 实现 `umb_fix_fixture_index_in_sync`：`_index.csv` 与实际文件扫描一致。
  6. 同文件实现 `umb_fix_every_inventory_id_is_covered`：每个 `UMB-NNNN` 至少被 fixture `COVERS` 或 sentinel coverage 覆盖，D 类例外。
  7. 同文件实现 `umb_fix_every_bucket_has_at_least_one_pos_and_one_neg`：A 类只要求 sentinel test，不强制 negative fixture。
  8. 同文件实现 `umb_fix_spec_coverage_matrix_in_sync`：矩阵中 fixture 引用真实存在，planned 状态在 U5 完成后必须清空或转 ignore/active。
  9. 同文件实现 `umb_fix_no_forbidden_terms_in_neg_messages`：negative `EXPECT-ERROR` 不含 forbidden terms。
  10. `crates/scoopc/src/audit/sentinel_tests.rs` 实现 `umb_fix_helper_invariant_sentinel_tests_present`：每个 A 类 bucket 至少一个 `#[should_panic]` 或等价 sentinel test。
  11. 在 `crates/scoopc/src/lib.rs` 中以 `#[cfg(test)]` 接入 audit module。
  12. 若 U1 已实现部分测试，本任务负责补齐、改名为计划中的稳定测试名，并使 `cargo test -p scoopc audit::` 全绿。
- 必须遵从的约束：
  - baseline test 不得依赖网络、当前工作目录以外文件或交互输入。
  - 测试失败消息必须指出具体文件、bucket、UMB id 或 fixture path。
  - 不通过放宽断言掩盖 U1-U5 数据不一致；发现不一致应修数据或文档。
- 验证：
  1. `cargo test -p scoopc audit:: -- --nocapture`
  2. `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  3. `cargo run -p scoop -- test tests/fixtures/umb_fix/`
  4. `cargo test --all --all-targets`
- 完成条件：
  - 10 条 baseline test 全部存在且通过。
  - 任意 inventory 漂移、bucket md 数字漂移、fixture index 漂移、spec matrix 悬空、禁词违规都会导致测试失败。
- 依赖：U5-T03
- 完成记录：待填写

### [TODO] U6-T02：退场标注 + 计划自检

- 参考：
  - [`PLAN.md`](./PLAN.md) §6 U6-T02、§7
  - [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §12
- 目标：
  - 完成本轮 doc-and-test only 计划的交付闭环，并把 P7/P8 production 修复移交给 `UnsupportedMainBody_DONE.md`。
- 必须实现的内容：
  1. 确认 `audit/UMB_inventory.csv`、`audit/UMB_inventory_schema.md`、36 份 bucket md、36 份 strategy md、spec matrix、`tests/fixtures/umb_fix/**`、`crates/scoopc/src/audit/**` 全部存在。
  2. 确认 `cargo test -p scoopc audit::` 全绿。
  3. 确认 `cargo run -p scoop -- test tests/fixtures/umb_fix/` 通过，允许 `IGNORE-UNTIL-FIX` 但必须由 runner 明确 skip / xfail。
  4. 在 `UnsupportedMainBody_FIX.md` §12 标注 `[DONE]`，并按 `PLAN.md:400` 追加 `// PLAN-MD: see PLAN.md (this repo root) for execution tracking` 或等价说明。若该注释格式不适合 Markdown，先更新 `PLAN.md` 再执行。
  5. 创建 `UnsupportedMainBody_DONE.md`，只写头部、P7/P8 退场标准引用和当前 inventory count，不实现 production 修复计划细节。
  6. 最终更新本 `TODO.md` 顶部“当前状态”为完成，并在 U6-T02 完成记录中列出全量验证命令结果。
- 必须遵从的约束：
  - 不在 U6-T02 顺手删除或改写 production `UnsupportedMainBody`。
  - 如果任何退场判据未满足，不得把 `UnsupportedMainBody_FIX.md` 标 `[DONE]`。
- 验证：
  1. `cargo test -p scoopc audit:: -- --nocapture`
  2. `cargo run -p scoop -- test tests/fixtures/umb_fix/`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test`
- 完成条件：
  - `PLAN.md` §7 的 10 条判据全部满足。
  - `UnsupportedMainBody_FIX.md` §12 标 `[DONE]`。
  - `UnsupportedMainBody_DONE.md` 占位落地。
- 依赖：U6-T01
- 完成记录：待填写
