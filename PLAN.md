# Scoop：UnsupportedMainBody 收口计划（doc-and-test only）

> 生成时间：2026-05-18
> 设计基线：[`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md)（2026-05-18 落项）
> 当前状态：待开始
> 上一轮（closure capture 修正 + sealed interface + 配套库类型）已完成；归档于 [`docs/archive/plans/PLAN-closure-fix.md`](./docs/archive/plans/PLAN-closure-fix.md) 与 [`docs/archive/plans/TODO-closure-fix.md`](./docs/archive/plans/TODO-closure-fix.md)。
> 行号说明：下文以本计划生成时点的文件路径与符号名为准；后续若行号漂移，优先按文件路径、符号名、bucket 编号 / inventory id 定位。

## 0. 工作原则

- **本计划是 doc-and-test only**：不动任何 production 代码。所有产出限于 `.md` 文档、`.scoop` fixture、`.stdout` expected output、`.csv` / `.json` 数据表，以及仅由这些数据驱动的 `#[cfg(test)]` baseline 单元测试。production 修复（删除 / 改写 `LlvmEmitError::UnsupportedMainBody` 站点）由后续 P7 计划承接，本计划只负责"提供测试用例 + 把每一处归档可寻址"。
- **不允许"兜底报错"**：每一处 `UnsupportedMainBody` 必须归属于以下三类之一——`FrontendReject`（前端早拒）、`InternalBugSentinel`（不可达断言）、`RealImpl`（真正实现缺失）。任何"不知道归到哪里"的 entry 直接判为本计划的工作量未完成，不允许 `expected_class=TBD` 残留进入合并。
- **spec 是覆盖完备性的最终标准**：fixture set 必须能与 `docs/spec/language_spec-part{1..6}.md` 章节做双向回链；inventory 中除 helper-invariant 外，每条 entry 都要有非空 `spec_anchor`。
- **既有 baseline 不破坏**：`pipeline_user_visible_failure_policy_*` 系列测试当前冻结了 `STALE_UNSUPPORTED_MAIN_BODY_COUNTS = 637`、源码总出现 `1 277` 等基线数字。本计划新增的对照样本（pos/neg fixture）若会改动这些数字，**必须**在同一 PR 同步更新基线常量并写明原因；不允许"意外漂移"。
- **inventory 是计划的唯一真值**：每一处 `UnsupportedMainBody { kind: ... }` 都要拿到一个稳定 ID（`UMB-NNNN`）、一个 bucket（`B-XX`）、一个 spec 锚（除 helper invariant），并在 `audit/UMB_inventory.csv` 落地。CSV 与源码之间由自动化脚本双向 diff，不允许手工维护。
- **bucket 边界 = 修复责任分配**：bucket 划分遵循"应该由谁保证不可达"——helper-invariant、upstream contract、real implementation、spec uncovered 四类一级分组（参见 §3 / §4），同一条 entry 唯一归属一个 bucket，模糊归属在 `notes` 字段记录第二候选。
- **D 类（spec uncovered）允许独立 release**：async / generator / yield 等 spec 当前未定义的 surface 对应的 inventory entry 可以暂留，在退场判据中独立计分；不强制本计划把它们的 spec 工作一并完成。
- **本计划不引入任何"暂时性 failing fixture"**：仓库内禁止保留 failing fixture（与上一轮 reshape 同条铁律）。`tests/fixtures/umb_fix/**` 中所有 fixture 要么 active 通过、要么明确标注 `IGNORE-UNTIL-FIX:B-XX` 并由 fixture runner 自动 skip / xfail——这种状态只用于 C 类 bucket 的 happy-path fixture 在 production 修复落地前的过渡。
- **`crates/scoopc/src/audit/` 必须 `#[cfg(test)]` 限定**：禁止参与 production codegen 链路。如果 cargo 结构需要它单独成 crate，则建立 `crates/scoopc-audit/` 并在 workspace 中显式 `dev-dependency` only。

## 1. 当前判断

### 1.1 实测基线（2026-05-18）

| 指标 | 数值 | 来源 |
|---|---|---|
| `LlvmEmitError::UnsupportedMainBody {` 在 `crates/scoopc/src/llvm/codegen/` 出现总数 | **1 277** | `git grep -n "UnsupportedMainBody {"` |
| 涉及文件数 | **60** | 同上 |
| `kind:` 字面量去重总数 | **964** | inventory grep + sort -u |
| 只出现一次的 `kind:` 字面量 | **825** | 同上 |
| `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 冻结条目（production 路径） | **637** | `crates/scoopc/src/pipeline_user_visible_failure_policy.rs` |
| `crates/scoopc/src/llvm/codegen_gap_inventory.rs` 已登记 entry | **21** | 同文件 |
| 错误枚举定义位置 | `crates/scoopc/src/llvm/mod.rs:135-169` | 源码 |
| 设计语义 | "compiler bug：LLVM 主 codegen 收到本不应抵达的节点（contract drift）" | 同上 |
| 实际用法 | 当作"上游各阶段的兜底层"，事实上是"一处一个 ad hoc bug 标签" | grep 结果 |

### 1.2 错误语义与现状偏离

- 设计上 `UnsupportedMainBody` 表示"内部不变量被打破"——upstream pipeline 应保证不可达。
- 现状是 60 个 codegen 文件里有 1 277 个不同的"兜底报错"分支，覆盖 **964** 种不同的 `kind:` 标签，其中 **825** 个只出现一次。这意味着每条 entry 的 root cause 都需要单独分析，没有"批量替换"的捷径。
- production 路径冻结的 637 条全部由 `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 守住——这也是本计划退场判据 P7-#3 的对账锚点。

### 1.3 既有审计资产与缺口

| 资产 | 位置 | 现状 | 本计划是否扩展 |
|---|---|---|---|
| 错误枚举定义 | `crates/scoopc/src/llvm/mod.rs:135-169` | 已存在 | 不动 |
| failure 分类基线 + 冻结计数 | `crates/scoopc/src/pipeline_user_visible_failure_policy.rs` | 已存在 | 本计划只读，P7 阶段才改写 |
| codegen-stage gap inventory | `crates/scoopc/src/llvm/codegen_gap_inventory.rs` | 21 条 entry | 与新 `audit/UMB_inventory.csv` 做交叉验证（baseline test #2） |
| 历史 gap ledger | `docs/archive/designs/PIPELINE_GAPS.md` | 历史快照 | 引用但不修改；P8 退场后再追加段落 |
| 全量 inventory（CSV）| `audit/UMB_inventory.csv` | **不存在** | 本计划 P1 创建 |
| bucket 文档（36 份）| `audit/UMB_categories/B-XX.md` | **不存在** | 本计划 P2 创建 |
| spec 覆盖矩阵 | `audit/spec_coverage_matrix.md` | **不存在** | 本计划 P3 创建 |
| 修复策略草案（36 份）| `audit/strategies/B-XX.md` | **不存在** | 本计划 P4 创建 |
| fixture 集 | `tests/fixtures/umb_fix/**` | **不存在** | 本计划 P5 创建 |
| baseline 测试 | `crates/scoopc/src/audit/**` (cfg(test)) | **不存在** | 本计划 P6 创建 |

### 1.4 bucket 候选清单（最终以 P1 inventory 为准）

参见 [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §3.2 的 36 个 bucket（`B-01` ~ `B-36`），按一级类划分：

- **A. Helper invariant**（codegen 内部 `expect!` / `unreachable!` 集中处理）：B-01、B-17（部分）、B-35（部分）。
- **B. Upstream contract**（前端 / typecheck / HIR / MIR strict verifier 拒绝）：B-02 ~ B-08、B-11、B-14（部分）、B-15、B-16、B-18 ~ B-23、B-25 ~ B-34、B-35（部分）。
- **C. Real implementation**（codegen 缺失合法形状支持）：B-09（部分）、B-10、B-12、B-13、B-14（部分）、B-24（部分）。
- **D. Spec uncovered**（async / generator / yield 等 spec 当前未定义）：B-24（部分）、B-36。

> 注：bucket 跨多个一级类是常态（例如 B-09 既有 B 又有 C，B-17 既有 A 又有 B），细分到 entry 级别由 `expected_class` 字段区分。

### 1.5 spec 覆盖现状

`docs/spec/language_spec-part{1..6}.md` 描述的语法特性目前**没有**双向回链——既无法从 inventory entry 反查 spec 章节，也无法从 spec 章节正向枚举对应 fixture。spec part 4 中的 async / generator / yield 是已知的"spec 未定义"区段（D 类 bucket 来源）；其余 5 个 part 的覆盖情况要等 P3 矩阵编制完毕才能给出准确判断。

## 2. 设计目标

1. **每一处 `UnsupportedMainBody` 可寻址**：通过稳定 `UMB-NNNN` ID + bucket + spec 锚 + expected_class 四元组定位，CSV ⟷ 源码双向 diff baseline test 守住。
2. **每一处 `UnsupportedMainBody` 可归类**：36 个 bucket 全部成文（`audit/UMB_categories/B-XX.md`），含 symptom / root cause / spec linkage / expected post-fix class / fix strategy outline / fixture pointer / open questions 七段。
3. **每一处 `UnsupportedMainBody` 可验证**：本计划 P5 阶段提供的 fixture set 必须满足"该 entry 在 P7 阶段被改写为 `FrontendReject` / `InternalBugSentinel` / `RealImpl` 之一"的可机器验证条件——也就是 fixture 在 P7 修复完成后能从"`IGNORE-UNTIL-FIX`"自动转 active，或在改写为 `unreachable!` 后由 sentinel test 直接命中。
4. **spec 覆盖矩阵闭环**：spec part 1-6 的每一节至少有一条 fixture 对应（除非显式标 `INTENTIONALLY-EMPTY: <spec 原句引用>`）；inventory 每条 entry 都能映射回 spec 锚（除 helper-invariant 外）。
5. **A 类先抽 helper**：B-01、B-17（部分）等机械重复 entry 在 P4 策略文档中先设计统一 helper（`MainCodegen::expect_insert_block` 等），避免 P7 阶段单点替换造成大量噪声。
6. **B 类必有 upstream gate**：每条 B 类 entry 在 P1 inventory 中标明 `upstream_gate`（typecheck / hir / mir / mir.materialize / strict verifier 之一），P5 中至少有一条 negative fixture 验证该 gate 真的会拒绝畸形输入。
7. **C 类先有 happy-path fixture**：每个 C 类 bucket 必须给出最小可行 fixture，且在该 fixture 通过之前，codegen 处的 `UnsupportedMainBody` **不允许**移除——本计划负责把 fixture 标注为 `IGNORE-UNTIL-FIX: B-XX`，由 fixture runner 自动 skip / xfail。
8. **D 类必有 spec 立场**：async / generator / yield 等 spec 未定义 surface，对应 inventory entry 在矩阵中显式标 `INTENTIONALLY-EMPTY: <spec 原句>`；P5 写 frontend-reject 负例 + 锁定文案，本计划不要求把 spec 一并补齐。
9. **baseline 测试 10 条全过**：CSV ⟷ 源码 / bucket 总数 / spec 锚 / class 分布 / fixture index / inventory id 全覆盖 / pos+neg 配齐 / spec 矩阵 in-sync / 禁词 / sentinel test 数（详见 §7.1 of `UnsupportedMainBody_FIX.md`）全部通过。
10. **退场计数器就绪**：`audit/UMB_inventory.csv` 行数本身即是 P7/P8 的退场计数器；本计划交付的 baseline test #1 同时承担"P7 修复进度"的 daily diff 角色。

## 3. inventory schema 与 bucket 划分

### 3.1 inventory 主表 schema

`audit/UMB_inventory.csv` 字段（与 [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §2.3 对齐）：

| 字段 | 含义 | 示例 |
|---|---|---|
| `id` | 稳定编号 `UMB-<NNNN>`，按 file+line 排序生成 | `UMB-0042` |
| `file` | 仓库相对路径 | `crates/scoopc/src/llvm/codegen/mir_body/types.rs` |
| `line` | 1-based 行号 | `74` |
| `kind` | `kind:` 字面量原文 | `pass MIR local type` |
| `route` | `RawMirLlvm` / `EffectLoweredLlvm` / `Both` / `Helper` | `Both` |
| `surface` | 触发点表达层 (`stmt`/`rvalue`/`terminator`/`type`/`builder`/`intrinsic`) | `type` |
| `bucket` | §3.2 分类编号 `B-XX` | `B-02` |
| `expected_class` | `FrontendReject` / `InternalBugSentinel` / `RealImpl` | `InternalBugSentinel` |
| `spec_anchor` | 关联 spec 章节锚（`;` 分隔） | `part2#9-泛型;part3#4-调用` |
| `upstream_gate` | 期望 gate（缺则 `TBD`） | `MIR strict verifier: type completeness` |
| `existing_fixture` | 已有覆盖 fixture | `tests/fixtures/run-pass/closure_env_basic.scoop` |
| `notes` | 短备注 / 第二候选 bucket | `cross-TypeStore equivalent fallback` |

schema 文档 `audit/UMB_inventory_schema.md` 需要逐字段列出取值域、合法性规则、对账规则。

### 3.2 bucket 编号约束

- `B-01` ~ `B-36` 为本计划稳定标识；除非 P1 inventory 完成后发现确有合并 / 拆分必要，否则不得新增、不得修改编号。
- 合并 / 拆分必须更新本文档 §1.4，且在 PR 描述里逐条说明影响范围。
- 每条 inventory entry 唯一归属一个 bucket，第二候选记入 `notes`。

### 3.3 fixture 命名约束

`tests/fixtures/umb_fix/B-XX-<slug>/` 目录布局：

```
tests/fixtures/umb_fix/
├── B-01-builder-invariant/
│   ├── _README.md
│   ├── pos_<spec_anchor>.scoop
│   ├── pos_<spec_anchor>.stdout
│   └── neg_<spec_anchor>.scoop      # EXPECT-ERROR 形式
├── B-02-mir-local-type/
│   └── ...
├── B-XX/
└── _index.csv
```

`_index.csv` schema、fixture 头部规范见 `UnsupportedMainBody_FIX.md` §6.1 / §6.3，本计划不再展开。

## 4. baseline 测试矩阵

P6 阶段必须新增的 10 条 baseline 测试（位置：`crates/scoopc/src/audit/`，全部 `#[cfg(test)]`）：

| 编号 | 测试名 | 守住的不变量 |
|---|---|---|
| #1 | `umb_inventory_csv_in_sync` | CSV ⟷ 源码 grep 结果完全一致；行数 == 1 277（基线值） |
| #2 | `umb_inventory_buckets_total` | 每个 bucket 的 entry 数 == bucket md 表头声明的数；总和 == 1 277 |
| #3 | `umb_inventory_each_entry_has_spec_anchor_or_helper_marker` | 每条 entry 要么 `spec_anchor` 非空，要么 `expected_class=InternalBugSentinel` 且 `spec_anchor=N/A:helper-invariant` |
| #4 | `umb_inventory_class_distribution` | 三类 entry 数与 bucket md 中 `Expected post-fix class` 段的数字对账 |
| #5 | `umb_fix_fixture_index_in_sync` | `tests/fixtures/umb_fix/_index.csv` ⟷ 实际目录扫描结果一致 |
| #6 | `umb_fix_every_inventory_id_is_covered` | 每个 `UMB-XXXX` 至少出现在一条 fixture 的 `// COVERS:` 行（D 类例外） |
| #7 | `umb_fix_every_bucket_has_at_least_one_pos_and_one_neg` | A 类 bucket 只要求 sentinel test，不强制 negative fixture |
| #8 | `umb_fix_spec_coverage_matrix_in_sync` | `audit/spec_coverage_matrix.md` 每行的 fixture 引用都真实存在 |
| #9 | `umb_fix_no_forbidden_terms_in_neg_messages` | negative fixture 的 `EXPECT-ERROR` 文案不含 `FRONTEND_REJECT_FORBIDDEN_TERMS`（"后端" / "backend" / "LLVM" / "codegen" / "UnsupportedMainBody"） |
| #10 | `umb_fix_helper_invariant_sentinel_tests_present` | `crates/scoopc/src/audit/sentinel_tests.rs` 中每个 A 类 bucket 至少一个 `#[should_panic]` 单测 |

> baseline test #1 同时承担 P7 阶段的 daily diff 角色：删一条 production `UnsupportedMainBody` → CSV 减一行 → 测试断言 `len == expected_count` 强制同步。expected_count 在 P1 完成时锁定为 1 277，P7 阶段每个 PR 显式调减。

## 5. 顺序总览

```
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

依赖说明：

- U0 早于其他全部阶段——锁定基线 + 摸底事实。
- U1（inventory）→ U2（bucket）→ U3（matrix）严格串行：后者依赖前者数据。
- U4（策略）与 U5（fixture）原则上可并行——它们都依赖 U2/U3 的数据但彼此独立。但 U5-T02 / U5-T03 的"反样本必须命中真实 frontend gate"约束依赖 U4 写明的 gate 位置，因此 U4 略先于 U5-T02。
- U6 在 U1-U5 数据稳定后串接，最后通过自检测试把整条链锁住。
- 整个 U2 / U4 阶段允许多人并行做 36 份 md 中的不同子集；只要 §3.2 的 bucket 编号不变，分工可灵活。

## 6. 分阶段计划

### U0. 摸底 + baseline 冻结

#### U0-T01. 现状摸底与基线冻结

参考：

- [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §0
- `crates/scoopc/src/llvm/mod.rs:135-169`
- `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`
- `crates/scoopc/src/llvm/codegen_gap_inventory.rs`
- `docs/archive/designs/PIPELINE_GAPS.md`

目标：

- 把 §1.1 实测基线表的 7 个数字逐项 reproduce 一遍——`git grep -c "UnsupportedMainBody {" -- crates/scoopc/src/llvm/codegen/`、文件去重、`kind:` 字面量去重、单次出现统计、`STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 当前值、`codegen_gap_inventory.rs` 长度——任一数字与本文 §1.1 不符，必须更新本文档而非偷调脚本。
- 列出 `crates/scoopc/src/llvm/codegen/` 60 个文件清单，按 `route`（`RawMirLlvm` / `EffectLoweredLlvm` / `Both` / `Helper`）粗分组——后续 U1-T01 自动化生成时按此排序生成 `UMB-NNNN`。
- 抽样 10 个 entry（每个一级类至少 2 个）做 root cause hypothesis 预演——把 §1.4 的 36 bucket 候选与抽样 entry 做对账，确认 bucket 划分对人工 reviewer 可解释。
- 确认 `audit/` 目录在仓库根创建（本任务**仅**创建空目录 + `.gitkeep`，正式产出物从 U1 开始）。

退场标准：

- §1.1 表格全部确认；
- 60 个 codegen 文件清单落地为 `audit/_baseline_files.txt`；
- 抽样 10 个 entry 的初步 root cause + bucket 归属落地为 `audit/_baseline_sampling.md`（仅 U0 阶段读，U1 完成后归档到 `docs/archive/`）。

### U1. P1 — Inventory 快照

#### U1-T01. inventory 脚本 + CSV 主表

实现 [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §2 全部内容：

- 在 `crates/scoopc/src/audit/umb_inventory.rs`（`#[cfg(test)]`）写源码 grep → CSV 重建 + 与磁盘 CSV diff 的双向校验逻辑。
- CSV 字段按 §3.1（本文档）/ §2.3（源文档）schema；按 file+line 排序生成 `UMB-NNNN`。
- 第一次跑必须输出 1 277 行；任一字段值缺失视为脚本不完整。
- 三个 bin 子命令 `cargo run -p scoopc --bin umb-audit -- list/diff/stats` 暂以 `#[cfg(test)]` 入口或独立 bin（实现时择一，记入完成记录）。

退场标准：

- `audit/UMB_inventory.csv` 落地，行数 == 1 277；
- `cargo test -p scoopc audit::umb_inventory` 通过；
- 每条 entry 的 `bucket` ∈ §1.4 的 36 个编号之一（`bucket=TBD` 残留 == 任务未完成）；
- 每个 `kind` 字面量在 CSV 中出现次数 == 实际源码中出现次数。

#### U1-T02. inventory schema 文档 + 索引子命令

- 写 `audit/UMB_inventory_schema.md`：逐字段列出取值域 / 合法性规则 / 对账规则 / `bucket` 与 §1.4 的对照表 / `expected_class` 三选一定义 / `spec_anchor` 多值规范。
- 完成 `umb-audit` 三个子命令的功能：
  - `list`：按 bucket 列 entry，支持 `--bucket B-02` 过滤；
  - `diff`：CSV ⟷ 源码 grep diff（核心）；
  - `stats`：每 bucket entry 数 / 每 class entry 数 / 每文件 entry 数。

退场标准：

- schema md 落地，所有字段都有取值域定义；
- 三个子命令 `cargo run -p scoopc --bin umb-audit -- {list,diff,stats}` 在干净 checkout 上跑通；
- baseline test #1（CSV in-sync）在 U6 阶段直接复用此脚本。

### U2. P2 — 成因分析（36 个 bucket）

#### U2-T01. bucket 分组确认 + md 表头声明

- 基于 U1 完成后的 CSV，把每个 bucket 的 entry 数 / 一级类分布 / `kind` 标签分布写入 `audit/UMB_categories/_overview.md`：表格列 `bucket / 名称 / 一级类 / entry 数 / 主要 kind 标签前 5 条`。
- 任一 bucket entry 数为 0：要么把该 bucket 从 §1.4 删除（合并到相近 bucket）+ 在本文档同步；要么承认 §1.4 列表与实际 inventory 不一致，重新评估 bucket 划分。
- 36 份 `audit/UMB_categories/B-XX.md` 创建空骨架，含七段标题（symptom / root cause / spec linkage / expected post-fix class / fix strategy outline / fixture pointer / open questions）+ 表头声明（`本 bucket entry 数：N`，与 inventory 对账）。

退场标准：

- `_overview.md` 每个 bucket entry 数与 CSV 完全一致；
- 36 份骨架 md 全部存在；
- 任何在本任务中拆 / 合的 bucket 决策同步更新本计划 §1.4 + `UnsupportedMainBody_FIX.md` §3.2。

#### U2-T02. 36 份 bucket md 主体

每份 `audit/UMB_categories/B-XX.md` 必须含（[`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §3.3）：

1. **Symptom**：从 inventory 抽出该 bucket 下所有 `(file, line, kind)` 表 + 三个最具代表性的源码片段（含上下文 ±10 行）。
2. **Root cause hypothesis**：上游哪个阶段的什么不变量缺失。
3. **Spec linkage**：所有相关 `language_spec-partN#section`。
4. **Expected post-fix class**：`FrontendReject` / `InternalBugSentinel` / `RealImpl` / `D-pending`；同 bucket 内不同 entry 落到不同类时分行枚举。
5. **Fix strategy outline**：高层策略一句话（具体策略草案在 U4-T01）。
6. **Fixture set pointer**：指向 `tests/fixtures/umb_fix/B-XX/...`（U5 阶段填实）。
7. **Open questions**：未解决的设计 / 规范问题。

退场标准：

- 36 份 md 全部成文；
- 每条 inventory entry 都有 bucket 归属（CSV-side 与 MD-side 双向校验脚本通过）；
- 每个 bucket md 中 `Expected post-fix class` 段的数字之和 == 该 bucket inventory entry 数。

### U3. P3 — Spec 覆盖矩阵

#### U3-T01. `audit/spec_coverage_matrix.md`

参考：`UnsupportedMainBody_FIX.md` §4 全部、`docs/spec/language_spec-part{1..6}.md`。

- 按 spec 6 个 part 编排，每节给出表格（`Spec 锚 / 语法特性 / 现有正例 / 现有负例 / 新增正例 / 新增负例 / 关联 buckets / 备注`）。
- spec part 4 的 async / generator / yield 区段必须显式标 `INTENTIONALLY-EMPTY: <spec 原句>` + 关联 D 类 bucket。
- spec part 1-3、5-6 各章节的"现有"两列扫一遍仓库现存 fixture 自动填充（`tests/fixtures/run-pass`、`typecheck`、`mir_lowered`、`hir`、`parse`、`resolve`、`runtime_gc` 等），"新增"两列填 U5 阶段计划落地的 fixture 路径占位（U5 完成后回头补实际路径）。
- inventory 中每条 entry 的 `spec_anchor` 字段必须能在矩阵中找到对应行（除 helper-invariant）。

退场标准：

- 矩阵每个 spec section 的"现有 + 新增"两列至少有一条 fixture（否则显式 `INTENTIONALLY-EMPTY: <spec 原句>`）；
- 矩阵中每个 `bucket` 链接均闭环（在 §1.4 / 36 份 bucket md 中存在）；
- 没有 inventory entry 找不到 spec 锚（除 helper-invariant）；
- baseline test #8 草稿：把矩阵每行的 fixture 引用 list 出来，准备在 U6 阶段做 in-sync 校验。

### U4. P4 — 修复策略草案（36 份）

#### U4-T01. 36 份 `audit/strategies/B-XX.md`

每份策略 md 按通用模板（`UnsupportedMainBody_FIX.md` §5.1）：

```
# B-XX 修复策略

## 上游契约
- 谁负责（typecheck / hir / mir / mir.materialize / strict verifier / codegen helper）
- 契约形式（强类型 invariant / impossible state / explicit gate）

## 落地路径
- A. helper invariant：抽 helper / 集中 unreachable! / 删该 bucket 全部站点
- B. upstream contract：上游加 explicit reject / strict verifier baseline / codegen 改 unreachable!
- C. real implementation：依 P5 fixture 驱动实现 / 完成后 codegen 走正常分支
- D. spec uncovered：立项 spec follow-up / 当前阶段以 FrontendReject 拒绝

## 验证锚
- 引用 §6 fixture 集 tests/fixtures/umb_fix/B-XX/**
- 列出退场标准（counts、inventory diff、verifier baseline）
```

A 类（B-01、B-17 部分、B-35 部分）特别要求：先设计统一 helper（命名建议见 `UnsupportedMainBody_FIX.md` §5.2，`MainCodegen::expect_insert_block` 等），避免 P7 单点替换噪声。

B 类（绝大多数）特别要求：每条 entry 必须指定**唯一一个** `upstream_gate`，gate 位置候选：

- `crates/scoopc/src/typecheck/**`
- `crates/scoopc/src/hir/lower/**`
- `crates/scoopc/src/mir/lower.rs` / `crates/scoopc/src/mir/materialize/**`
- `crates/scoopc/src/mir/{verify,strict_verify}.rs`（若存在；否则 P7 阶段创建，本计划只占位）

C 类特别要求：每个 C 类 bucket 给出最小 happy-path fixture（在 U5-T02 / U5-T03 中落地），fixture 标记 `IGNORE-UNTIL-FIX: B-XX`。

D 类特别要求：在 spec 补齐之前不允许进入 P7；本计划只负责 frontend-reject 负例。

退场标准：

- 36 份策略 md 全部成文；
- 每份策略中"上游契约 / 落地路径 / 验证锚"三段均不空白；
- 每条 B 类 entry 的 `upstream_gate` 字段在 inventory CSV 中已填实（不再是 `TBD`）。

### U5. P5 — Fixture 集合

#### U5-T01. fixture 目录骨架 + `_index.csv`

- 按 §3.3 创建 `tests/fixtures/umb_fix/B-01-builder-invariant/` ~ `tests/fixtures/umb_fix/B-36-<slug>/` 共 36 个目录；每目录创建 `_README.md`（指向对应 bucket md + 策略 md）。
- `tests/fixtures/umb_fix/_index.csv` 创建表头（`fixture_path / bucket / kind / spec_anchor / umb_ids / status / notes`），暂时只填本任务能预知的占位条目。

退场标准：

- 36 个目录全部存在 + `_README.md` 落地；
- `_index.csv` 表头与 schema 一致；
- baseline test #5 草稿：扫描目录 → diff `_index.csv` 的逻辑准备就绪。

#### U5-T02. spec part 1-6 fixture 主体

按 [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §6.4 列出的 1-48 条 fixture（按 spec part 切分）逐项落地，每条 1 正例 + 至少 1 负例（除非 spec 明确说该形态总是合法）。

- **Part 1**（1-3）：词法、字面量、源结构。
- **Part 2**（4-12）：类型系统、Niche / 装箱、泛型、`with` copy-update。
- **Part 3**（13-26）：表达式 / 函数 / 模式 / 控制流 / `when` / cast / Range / Operator overloading / Class literal / 类型推断。
- **Part 4**（27-34）：effect 系统 / handler / try-catch-finally / RuntimeError / Entry point / async-generator-yield 反样本。
- **Part 5**（35-42）：编译期执行 / `comptime if/for` / 反射 intrinsic / splice field / Platform / RTTI / 注解 / `@Intrinsic` + sysroot。
- **Part 6**（43-48）：unsafe / safe region / `@NoGC` / raw pointer / FFI / GC.handle*。

每条 fixture 的头部规范（pos / neg）见 `UnsupportedMainBody_FIX.md` §6.3。fixture 撰写禁区（不允许新增 `UnsupportedMainBody` 站点 / 不允许使用 sysroot 之外的库 / `EXPECT-ERROR` 不含禁词 / 不允许 fixture 间 import）见同文 §6.5。

退场标准：

- 48 条 fixture 全部落地（pos + neg 配齐或显式标 spec-always-legal）；
- `_index.csv` 同步更新；
- 所有 negative fixture 的 `EXPECT-ERROR` 锁定文案 + 错误码；
- baseline test #6 / #7 / #9 在 U6 阶段直接验证此批 fixture。

#### U5-T03. bucket-driven 直接对账 fixture

按 [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §6.4 第 49 条要求：每个 §1.4 中列出的 36 个 bucket 至少有一条**专门**针对 inventory 中该 bucket 代表 entry 的 fixture（即使在 U5-T02 中已经覆盖了，也要在 `_index.csv` 中显式登记 `umb_ids`）。

- 每个 fixture 顶部注释 `// COVERS: UMB-0001, UMB-0042, UMB-0073` 列出旨在驱动消除的 inventory id 列表。
- 一条 fixture 多对一覆盖多个 entry 是被鼓励的（§11 风险 #4 对策）；只有当一条 fixture 无法物理同时触发多 entry 时才拆分。
- C 类 bucket 的 happy-path fixture 必须标 `IGNORE-UNTIL-FIX: B-XX` + 在 `_index.csv` 中 status 字段标 `ignore-until-fix:B-XX`。

退场标准：

- 每个 bucket 至少有一条专属 fixture；
- 每个 inventory id 至少出现在一条 fixture 的 `// COVERS:` 行中（D 类标 `D-pending` 例外）；
- baseline test #6（每 inventory id 都被覆盖）通过。

### U6. P6 — Baseline 测试

#### U6-T01. 10 条 baseline test 落地

按 §4 表格（与 `UnsupportedMainBody_FIX.md` §7.1 对齐）实现：

- `crates/scoopc/src/audit/umb_inventory.rs`：测试 #1 / #2 / #3 / #4。
- `crates/scoopc/src/audit/sentinel_tests.rs`：测试 #10（每个 A 类 bucket 至少一个 `#[should_panic]` 单测）。
- `crates/scoopc/src/audit/spec_coverage.rs`：测试 #5 / #6 / #7 / #8 / #9。

注意：

- `crates/scoopc/src/audit/` 整个 module 必须 `#[cfg(test)]` 限定（参见 §0 工作原则）；如果 cargo 结构需要它单独成 crate，则建立 `crates/scoopc-audit/` 并在 workspace 中显式标 `dev-dependency` only。
- 测试的"基线数字"（1 277、bucket 总数 36、各 bucket entry 数等）必须在 U1-T01 完成后即时锁定，不允许在 U6 阶段重新拍脑袋。

退场标准：

- `cargo test -p scoopc audit::` 全绿；
- 任意一条 fixture 损坏 / inventory 漂移 / spec 矩阵失同步 → 对应 baseline test 必败；
- 测试名稳定（P7 阶段 PR diff 直接看测试名变更确认涉及面）。

#### U6-T02. 退场标注 + 计划自检

- 在 `UnsupportedMainBody_FIX.md` 文件末尾追加一行 `// PLAN-MD: see PLAN.md (this repo root) for execution tracking`，并在 [`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) §12 标注 `[DONE]`（条件：本计划 §7 退场标准全部满足）。
- 自检脚本 `cargo test -p scoopc audit::` + `cargo run -p scoop -- test tests/fixtures/umb_fix/`（允许带 `IGNORE-UNTIL-FIX` 状态）一次性全跑过。
- 创建 `UnsupportedMainBody_DONE.md` 占位文件（接手 P7/P8 退场标准的追踪），仅写入头部 + 引用本计划 §7。

退场标准：

- 本计划 §7 全部判据通过；
- `UnsupportedMainBody_FIX.md` § 12 状态切换至 `[DONE]`；
- `UnsupportedMainBody_DONE.md` 占位落地。

## 7. 本计划的退场判据

当且仅当下列条件**全部**满足，本计划文档可标注 `[DONE]`：

1. `audit/UMB_inventory.csv` 落地，行数 == 1 277；
2. `audit/UMB_inventory_schema.md` 落地，schema 字段定义完整；
3. `audit/UMB_categories/B-01.md` ~ `audit/UMB_categories/B-36.md` 全部成文，每份七段齐全；
4. `audit/spec_coverage_matrix.md` 落地，spec part 1-6 全覆盖（除 `INTENTIONALLY-EMPTY` 引用 spec 原句外）；
5. `audit/strategies/B-01.md` ~ `audit/strategies/B-36.md` 全部成文，每份"上游契约 / 落地路径 / 验证锚"三段不空白；
6. `tests/fixtures/umb_fix/_index.csv` 与目录扫描结果一致；
7. `tests/fixtures/umb_fix/B-XX/**` 每个 bucket 至少一条专属 fixture，每个 inventory id 至少在一条 fixture 的 `// COVERS:` 中（D 类例外）；
8. `cargo test -p scoopc audit::` 全绿；
9. `cargo run -p scoop -- test tests/fixtures/umb_fix/` 通过（允许带 `IGNORE-UNTIL-FIX` 状态）；
10. `UnsupportedMainBody_FIX.md` § 12 标注 `[DONE]`，`UnsupportedMainBody_DONE.md` 占位落地。

P7（production 修复）/ P8（variant 物理删除）的退场判据由后续计划承接，本计划不涉及。

## 8. 兼容性 / 迁移影响

- `crates/scoopc/src/audit/` 是新增 module，全部 `#[cfg(test)]`，对 production codegen 链路零影响；workspace dependency tree 不变。
- `audit/UMB_inventory.csv`、36 份 bucket md、spec 矩阵、36 份策略 md 均为 doc 产出物，不影响构建。
- `tests/fixtures/umb_fix/**` 是新增 fixture 目录；fixture runner 须识别 `IGNORE-UNTIL-FIX:B-XX` 头部标注做 skip / xfail——若现有 runner 不支持此标记，本计划在 U5-T01 阶段把 runner 扩展任务作为子任务列入完成记录（runner 改动属于 test infrastructure，不算 production 代码）。
- `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` / `INTERNAL_BUG_SENTINEL_HITS` / `FRONTEND_REJECT_SURFACES` 三表本计划**只读**，不动；P7 阶段才会逐条调减。
- `pipeline_user_visible_failure_policy_*` 系列测试现冻结的 637 数字本计划不变；如果 U5 fixture 中负例触发了新的 frontend reject / sentinel hit 路径，必须在 U5-T02 / U5-T03 完成记录中显式登记，由 P7 阶段对账。
- 不引入任何 spec 改动；async / generator / yield 等 D 类 bucket 触及的 spec 缺口由后续 spec 计划承接，本计划仅以 `INTENTIONALLY-EMPTY` 形式登记。
- 不引入任何 sysroot 改动。

## 9. 风险与对策

- **inventory 漂移**：源码改动会冲击 CSV。
  - 对策：baseline test #1（`umb_inventory_csv_in_sync`）是强制 gate，PR diff 必须显式更新 CSV 才能合并。U1-T01 实现时把 `umb-audit diff` 子命令做成 CI 入口。
- **bucket 边界争议**：某些 entry 同时像 B-02 又像 B-09。
  - 对策：每条 entry 唯一 bucket；模糊归属优先归到"上游修复点更具体"的 bucket，并在 `notes` 字段记录另一候选。U2-T01 阶段对所有跨类 entry 留下决策记录。
- **spec 缺口**：D 类 bucket 触及 spec 未定义。
  - 对策：U3-T01 矩阵中 `INTENTIONALLY-EMPTY` 必须直接引用 spec 原句；遇到 spec 沉默时，U5-T02 阶段写 frontend-reject 负例并把策略归到 `BlockedOnSpec`。D 类不计入本计划 §7 判据 #1 的"清零"约束（但仍要在 #4 中登记 `INTENTIONALLY-EMPTY`），允许独立 release。
- **fixture 体量爆炸**：1 277 entry × 平均 2 fixture ≈ 上千文件。
  - 对策：每个 fixture 通过 `// COVERS:` 多对一覆盖 inventory id；只有当一条 fixture 无法物理同时触发多 entry 时才拆分。U5-T03 完成记录中给出"平均覆盖率"指标（每条 fixture 覆盖多少 entry），高于 5 视为预期。
- **bucket md 一致性**：36 份 md 易出现 symptom 表格 / class 分布数字与 CSV 不同步的情况。
  - 对策：baseline test #2 / #4 直接对账。U2-T02 阶段强制使用半自动模板：把 CSV 抽出对应 bucket 的 entry 表 → 直接贴进 md → 任何差异必须改 CSV 而非改 md。
- **spec 矩阵 fixture 引用悬空**：U3-T01 时 fixture 还没写，"新增"两列只能填占位；U5 完成后回头同步极易遗漏。
  - 对策：baseline test #8（`spec_coverage_matrix_in_sync`）守住实际存在性，U5-T02 / U5-T03 完成记录强制要求"每新增一条 fixture，同步更新矩阵"。
- **A 类 sentinel 测试设计成本**：B-01 等 helper-invariant bucket 在 fixture 层无法直接构造，要靠 `#[should_panic]` 单测模拟 IR 注入。
  - 对策：U4-T01 写 B-01 策略时一并设计 helper signature；U6-T01 测试 #10 严格按"每 A 类 bucket ≥ 1 sentinel test"落地——sentinel test 的注入路径建议复用现有 `crates/scoopc/src/llvm/codegen/...` 的 unit test mock，不另建 framework。
- **`audit/` 与 `crates/scoopc/src/audit/` 的命名混淆**：前者是仓库根 doc 产出目录，后者是 cfg(test) Rust module。
  - 对策：本计划 §3 / §4 / §5 / §6 全程使用绝对路径明确区分；U1-T01 / U6-T01 完成记录强制写明哪部分落在哪里。
- **fixture runner 不识别 `IGNORE-UNTIL-FIX:B-XX`**：现有 runner 行为未知。
  - 对策：U5-T01 开工时第一步勘测现有 runner（`tests/fixtures/run-pass/` 等历史标记机制），如不支持则把 runner 扩展任务作为子任务列入 U5-T01 完成记录。runner 改动是 test infrastructure，不算 production 代码（参见 §0 工作原则）。
- **CSV 行数基线 1 277 在 U1 实测时偏移**：源码自 2026-05-18 后可能继续微动，导致 U1-T01 跑出来不是 1 277。
  - 对策：以 U1-T01 实跑值为准，回头同步本文档 §1.1 / §3.1 / §4 baseline test #1 的"基线值"；任何调整必须在 PR 描述里写明 delta 来源（具体 commit）。
- **U2 / U4 多人并行的 md 风格漂移**：36 份 md 由不同人写易出现段落顺序 / 标题层级 / 表格列序不一致。
  - 对策：U2-T01 创建骨架时把七段 / 三段标题与表头格式固化为模板（在 `_overview.md` 里给出范例 B-01.md 的完整样本）；U6-T01 baseline test #2 / #3 / #4 隐含校验段落与字段数。
