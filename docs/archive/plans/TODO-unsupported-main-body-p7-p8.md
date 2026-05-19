# TODO（UnsupportedMainBody Production 修复计划，P7/P8）

> 生成时间：2026-05-18
> 计划基线：[`PLAN.md`](./PLAN.md)
> 上一阶段任务档案：[`TODO-1.md`](./TODO-1.md)
> 设计与 baseline：[`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md)、[`UnsupportedMainBody_DONE.md`](./UnsupportedMainBody_DONE.md)
> 当前状态：P7-0-T01、P7-0-T02、P7-A1、P7-A2、P7-A3、P7-A4、P7-B1、P7-B2.1、P7-B2.2、P7-B2.3、P7-B2.4、P7-B2.5、P7-B2.6、P7-B2.7、P7-B2.8、P7-B3.1、P7-B3.2、P7-B3.3、P7-B3.4、P7-B3.5、P7-C1、P7-C2、P7-C3、P7-C4、P7-C5、P8-T01、P8-T02 已完成；active=0，retired=1284；`LlvmEmitError::UnsupportedMainBody` 已删除，P8 归档完成。

## 全局约束

- 本 TODO 对应 `PLAN.md` 的 P7 production 修复与 P8 退场审计；P1-P6 doc-and-test only 阶段已经归档到 `PLAN-1.md` / `TODO-1.md`。
- 不允许新增 `LlvmEmitError::UnsupportedMainBody { ... }` 站点；不允许把后端兜底错误换成另一个兜底 diagnostic。
- 每个 production 修复必须以 `UMB-NNNN` 为对账单位，明确 retire 的 ID、bucket、expected_class 和 source file。
- 每个修复 PR 必须同步更新 active inventory、retired ledger、bucket 文档、fixture coverage、stale count baseline 和相关测试；不得只改 production 代码。
- `FrontendReject`、`InternalBugSentinel`、`RealImpl` 三类治理路径必须按 `audit/UMB_inventory.csv` 的 `expected_class` 执行；不能用 frontend reject 规避 C 类合法路径实现。
- D 类 spec-uncovered surface 当前按第一阶段基线走 `FrontendReject`；不在本轮顺手扩展 async/generator/yield 等未定义 spec。
- 每个任务完成后必须在对应“完成记录”段回写：改动范围、retired IDs、核心决策、验证结果、对 `PLAN.md` 完成条件的闭合情况。
- 工作区可能已有用户改动；执行任务时不得 revert 或覆盖无关改动。

## 固定定位清单

### 计划与审计文件

- `PLAN.md:43-79`：P7-0 audit/tooling 稳定化。
- `PLAN.md:81-94`：单个 production 修复 PR 的固定流程。
- `PLAN.md:96-117`：P7-A FrontendReject 退场。
- `PLAN.md:119-181`：P7-B InternalBugSentinel 退场。
- `PLAN.md:183-206`：P7-C RealImpl 退场。
- `PLAN.md:208-225`：P8 最终退场。
- `PLAN.md:237-245`：总完成判据。
- `audit/UMB_inventory.csv`：最终 active inventory，header-only，0 条；归档副本见 `docs/archive/audits/unsupported-main-body/UMB_inventory_final_empty.csv`。
- `audit/UMB_categories/_overview.md`：bucket entry 数和 class 分布总览。
- `audit/UMB_categories/B-XX.md`：每 bucket symptom、root cause、fixture pointer。
- `audit/strategies/B-XX.md`：每 bucket 上游契约和 P7 修复策略。
- `tests/fixtures/umb_fix/_index.csv`：P8 后长期 fixture coverage 索引；历史 UMB id 覆盖由归档 ledger 承接。
- `UnsupportedMainBody_DONE.md`：P7/P8 handoff 和最终退场记录位置。

### 工具与测试入口

- `crates/scoopc/src/bin/umb-audit.rs` 已在 P8-T02 删除；历史 ledger 和 empty inventory 归档于 `docs/archive/audits/unsupported-main-body/`。
- `cargo test -p scoopc audit:: -- --nocapture`：长期 `umb_fix` fixture coverage audit。
- `cargo test -p scoopc pipeline_gap_audit -- --nocapture`：P1-P6 audit baseline。
- `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`：用户可见失败策略和 stale count baseline。
- `cargo run -p scoop -- test tests/fixtures/umb_fix/`：UMB fixture 集合。
- `cargo test --all --all-targets`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`：阶段性全量验证。

### Production 关键位置

- `crates/scoopc/src/llvm/mod.rs`：`LlvmEmitError::UnsupportedMainBody` enum variant 和 diagnostic 映射已删除。
- `crates/scoopc/src/llvm/codegen/**`：无 `UnsupportedMainBody` production constructor。
- `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`：历史 stale UMB count 已删除，保留长期用户可见失败策略 guard。
- `crates/scoopc/src/audit/spec_coverage.rs`：P8 后保留的长期 `umb_fix` fixture coverage audit。

## 执行顺序总览

```text
P7-0-T01 stable ID + retired ledger
  -> P7-0-T02 active/retired countdown audit
      -> P7-A1..A4 FrontendReject retire
          -> P7-B1 helper invariant
              -> P7-B2.1..B2.8 core MIR/HIR/type/layout contract
                  -> P7-B3.1..B3.5 intrinsic/sysroot contract
                      -> P7-C1..C5 real implementation
                          -> P8-T01 final variant removal
                              -> P8-T02 archive + DONE record
```

依赖规则：

- P7-0 必须先于任何 production row 删除完成。
- P7-A 可在 P7-0 后按小 PR 推进，优先退场用户可见非法输入。
- P7-B1 先于大规模 InternalBugSentinel 迁移，用于固定 helper 风格。
- P7-B2 与 P7-B3 原则上可在不同文件族并行，但每个 PR 必须独立保持 audit/stale count 对账通过。
- P7-C 依赖相关 ABI、transport、closure、effect contract 稳定；B-10 不应早于 C1-C4 大规模推进。
- P8 只能在 active inventory 为 0、retired ledger 覆盖 1,284 个 initial ID 后执行。

## 每个修复 PR 的固定完成记录模板

执行任一 P7-A/P7-B/P7-C 任务时，在任务末尾补充：

```md
- 完成记录：
  - 改动范围：<files changed>
  - Retired IDs：<UMB-XXXX..>；数量 <N>；bucket <B-XX>；class <...>
  - 核心决策：<frontend gate / verifier contract / helper / real implementation>
  - Inventory/ledger：active <old> -> <new>；retired <old> -> <new>
  - Stale count：<file counts changed>；total <old> -> <new>
  - Fixture 状态：<active/ignore 状态变化>
  - 验证结果：<commands and results>
  - 闭合目标：<PLAN.md sections satisfied>
```

## P7-0：Audit / Tooling 稳定化

### [DONE] P7-0-T01：引入稳定 ID 与 retired ledger

- 参考：`PLAN.md:47-63`。
- 目标：删除源码 row 后，未删除的 `UMB-NNNN` 不重排。
- 必须实现：
  1. 保留当前 1,284 行 immutable baseline，例如 `audit/UMB_inventory_initial.csv`。
  2. 新增 retired ledger，例如 `audit/UMB_retired.csv`。
  3. retired ledger 字段至少包含 `id,bucket,expected_class,file,old_line,kind,retired_by,retired_reason,retired_at_notes`。
  4. 改造 `crates/scoopc/src/audit/umb_inventory.rs`，让 active inventory 从 baseline/上一版 CSV 继承稳定 ID。
  5. 支持 line drift 匹配：exact `(file,line,kind)`、唯一 `(file,kind,bucket,expected_class,surface)`、同组按 old/source line 顺序配对。
  6. 对无法唯一匹配的 row 报错，禁止自动重号。
  7. 校验 active IDs 与 retired IDs 互斥，且并集等于 initial 1,284 IDs。
- 必须遵从：本任务不 retire production row；只改 audit/tooling 和数据结构。
- 验证：
  1. `cargo run -p scoopc --bin umb-audit -- diff`
  2. `cargo run -p scoopc --bin umb-audit -- stats`
  3. `cargo test -p scoopc audit:: -- --nocapture`
- 完成条件：当前状态 active=1,284、retired=0；新增测试覆盖“模拟删除 row 后 remaining IDs 不重排”。
- 依赖：无。
- 完成记录：
  - 改动范围：新增 `audit/UMB_inventory_initial.csv` immutable baseline、`audit/UMB_retired.csv` 空 ledger；更新 `crates/scoopc/src/audit/umb_inventory.rs` stable ID 继承/校验逻辑；更新 `audit/UMB_inventory_schema.md`；同步记录 `memory/claude_plan.md`。
  - Retired IDs：无；数量 0；本任务不退场 production row。
  - 核心决策：active inventory 从 initial baseline 继承 `UMB-NNNN`；匹配顺序为 exact `(file,line,kind)`、唯一 `(file,kind,bucket,expected_class,surface)`、同组按 initial/source line 顺序配对；无法唯一匹配时报错。
  - Inventory/ledger：active 1,284 -> 1,284；retired 0 -> 0；initial baseline 1,284。
  - Stale count：未改 production fallback，stale count 不变。
  - Fixture 状态：未改 fixture active/ignore 状态。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，1284 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（total_entries=1284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（20 passed）；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:47-63` 与本任务完成条件；新增测试覆盖模拟删除 row 后 remaining IDs 不重排，并覆盖 line drift 顺序配对与歧义匹配报错。

### [DONE] P7-0-T02：把 audit 常量改成退场倒计时

- 参考：`PLAN.md:65-79`。
- 目标：P7 可以按 PR 递减 active count，而不是永远断言 1,284 active rows。
- 必须实现：
  1. 保留 `INITIAL_ENTRY_COUNT = 1_284` 或等价常量，只用于 `active + retired == initial`。
  2. 将 active count、literal kind count、dynamic kind count 改为随 active inventory 更新的 baseline。
  3. `umb-audit stats` 输出 active、retired、initial、by_class、by_bucket。
  4. `umb-audit diff` 可报告新增、删除、line drift、field drift，但不能因 active < initial 直接 panic。
  5. 更新 audit tests 的失败消息，使退场 PR 能定位具体 ID、bucket、file。
- 必须遵从：本任务仍不 retire production row。
- 验证：
  1. `cargo run -p scoopc --bin umb-audit -- stats`
  2. `cargo run -p scoopc --bin umb-audit -- diff`
  3. `cargo test -p scoopc audit:: -- --nocapture`
  4. `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
- 完成条件：当前状态显示 active=1,284、retired=0、initial=1,284；后续任务可只更新 retired IDs。
- 依赖：P7-0-T01。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/audit/umb_inventory.rs`、`crates/scoopc/src/bin/umb-audit.rs`、`crates/scoopc/src/audit/spec_coverage.rs`、`audit/UMB_inventory_schema.md`；同步记录 `memory/claude_plan.md`。
  - Retired IDs：无；数量 0；本任务不退场 production row。
  - 核心决策：保留 `INITIAL_ENTRY_COUNT = 1_284` 只用于 active + retired 与 initial 对账；active count、literal kind count、dynamic kind count 改为从当前 active inventory / source scan 派生；`umb-audit diff` 使用 diff-mode stable ID 匹配报告未对账的新增/删除而不因 active < initial 直接 panic；严格 audit 路径仍要求 active IDs 与 retired IDs 并集等于 initial。
  - Inventory/ledger：active 1,284 -> 1,284；retired 0 -> 0；initial 1,284。
  - Stale count：未改 production fallback，stale count 不变。
  - Fixture 状态：未改 fixture active/ignore 状态；fixture coverage audit 改为验证 active + retired countdown，不再要求 active ID 数恒为 1,284。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- stats` 通过（active_entries=1284、retired_entries=0、initial_entries=1284）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，1284 entries）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:65-79` 与本任务完成条件；后续退场 PR 可按 retired ledger 递减 active inventory，无需维持 1,284 active rows。

## P7-A：FrontendReject 退场（125 entries）

### [DONE] P7-A1：B-16 控制流 outside-of-context 早拒

- 参考：`PLAN.md:96-117`、`audit/strategies/B-16.md`、`audit/UMB_categories/B-16.md`。
- 范围：B-16，7 entries，`FrontendReject`。
- 目标：`break`、`continue`、`return` 的非法上下文在 parse/HIR/typecheck 阶段稳定拒绝，不进入 LLVM codegen。
- 必须实现：
  1. 用 `umb-audit list --bucket B-16` 锁定 7 个 ID。
  2. 补齐或确认 frontend/typecheck gate 覆盖 `break outside loop`、`continue outside loop`、`return outside function with return context`。
  3. 删除对应 codegen fallback，必要处改为 verifier 后的 `unreachable!`。
  4. 更新 active inventory、retired ledger、B-16 文档、fixture coverage 和 stale count。
- 验证：
  1. `cargo run -p scoopc --bin umb-audit -- list --bucket B-16`
  2. `cargo test -p scoopc audit:: -- --nocapture`
  3. `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  4. `cargo run -p scoop -- test tests/fixtures/umb_fix/B-16-control-flow-context/`
- 完成条件：B-16 active count 为 0；相关 negative fixture active 并通过。
- 依赖：P7-0-T02。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/codegen/control_flow.rs`、`crates/scoopc/src/llvm/codegen/stmt.rs`、`crates/scoopc/src/llvm/codegen/main/call.rs`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/B-16.md`、`audit/UMB_categories/_overview.md`、`audit/strategies/B-16.md`、`audit/spec_coverage_matrix.md`；激活并扩充 `tests/fixtures/umb_fix/B-16-control-flow-context/` 与 `_index.csv`；同步记录 `memory/claude_plan.md`。
  - Retired IDs：`UMB-0187`、`UMB-0188`、`UMB-0191`、`UMB-0192`、`UMB-0786`、`UMB-1263`、`UMB-1264`；数量 7；bucket `B-16`；class `FrontendReject`。
  - 核心决策：复用既有 `BreakNotInLoop`、`ContinueNotInLoop`、`ReturnNotInFunctionBody` frontend/typecheck gate；LLVM lowering 删除 B-16 `UnsupportedMainBody` fallback，改为上游 gate 后的 `unreachable!` invariant。
  - Inventory/ledger：active 1,284 -> 1,277；retired 0 -> 7；B-16 active 7 -> 0。
  - Stale count：`crates/scoopc/src/llvm/codegen/main/call.rs` `UnsupportedMainBody` 12 -> 11；tracked stale total 638 -> 637。
  - Fixture 状态：B-16 fixture directory 从 `IGNORE-UNTIL-FIX:B-16` 激活；新增 `neg_return_in_lambda_context.scoop`；B-16 retired IDs 改由 retired ledger 覆盖，active fixture `COVERS` 不再引用 retired IDs。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-16` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，1,277 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=1,277、retired=7、initial=1,284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-16-control-flow-context/` 通过（5 passed）；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:96-117` 与本任务完成条件；B-16 active count 为 0，相关 negative fixture active 并通过。

### [DONE] P7-A2：B-08/B-21 成员写入与 struct 字段负例早拒

- 参考：`PLAN.md:100-117`、`audit/strategies/B-08.md`、`audit/strategies/B-21.md`。
- 范围：B-08 FrontendReject rows 2 entries；B-21 FrontendReject rows 3 entries；合计 5 entries。
- 目标：不可写 target、immutable member store、unknown/missing struct field 在上游稳定拒绝。
- 必须实现：
  1. 用 `umb-audit list --bucket B-08` 与 `--bucket B-21` 锁定本任务的 `FrontendReject` IDs。
  2. 在 typecheck/HIR/MIR verifier 中补齐字段存在性、可写性、初始化完整性 gate。
  3. 删除对应 codegen fallback，保留必要 invariant 注释。
  4. 更新 inventory、retired ledger、B-08/B-21 文档、fixture coverage、stale count。
- 验证：
  1. `cargo test -p scoopc audit:: -- --nocapture`
  2. `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  3. `cargo run -p scoop -- test tests/fixtures/umb_fix/B-08-member-store/`
  4. `cargo run -p scoop -- test tests/fixtures/umb_fix/B-21-struct-fields/`
- 完成条件：B-08/B-21 的 FrontendReject active rows 为 0；相关 negative fixture active 并通过。
- 依赖：P7-0-T02。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/codegen/layout.rs`、`crates/scoopc/src/llvm/codegen/main/expr_value.rs`、`crates/scoopc/src/llvm/codegen/mir_body/aggregates.rs`、`crates/scoopc/src/llvm/codegen/mir_body/member.rs`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/B-08.md`、`audit/UMB_categories/B-21.md`、`audit/UMB_categories/_overview.md`、`audit/strategies/B-08.md`、`audit/strategies/B-21.md`、`tests/fixtures/umb_fix/_index.csv`、B-08/B-21 相关 fixtures 与 `memory/claude_plan.md`；同步 `layout.rs` line drift 影响的 B-06/B-20/B-22/B-36 文档行号。
  - Retired IDs：`UMB-1131`、`UMB-1142`、`UMB-0750`、`UMB-0863`、`UMB-0962`；数量 5；bucket `B-08`/`B-21`；class `FrontendReject`。
  - 核心决策：复用 typecheck 的 assignment/member-store 可写性 gate 与 struct field gate；LLVM lowering 删除用户面 `UnsupportedMainBody` fallback，改为上游 gate 后的 `unreachable!` invariant；剩余 B-08/B-21 active rows 均为后续 verifier/internal contract 任务处理的 `InternalBugSentinel`。
  - Inventory/ledger：active 1,277 -> 1,272；retired 7 -> 12；B-08 `FrontendReject` active 2 -> 0；B-21 `FrontendReject` active 3 -> 0。
  - Stale count：`crates/scoopc/src/llvm/codegen/mir_body/aggregates.rs` `UnsupportedMainBody` 31 -> 30；`crates/scoopc/src/llvm/codegen/main/expr_value.rs` 19 -> 18；`crates/scoopc/src/llvm/codegen/mir_body/member.rs` 50 -> 48；tracked stale total 637 -> 633；`layout.rs` 另删除 1 个 inventory fallback（不在 tracked stale list）。
  - Fixture 状态：B-08/B-21 frontend negative fixtures active 并通过；新增 `neg_immutable_member_store.scoop` 与 `neg_struct_literal_missing_field.scoop`；retired IDs 改由 retired ledger 覆盖，active fixture `COVERS` 不再引用 retired IDs。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-08` 通过（remaining active entries 4，均为 `InternalBugSentinel`）；`cargo run -p scoopc --bin umb-audit -- list --bucket B-21` 通过（remaining active entries 3，均为 `InternalBugSentinel`）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=1,272、retired=12、initial=1,284）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，1,272 entries）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-08-member-store/` 通过（4 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-21-struct-fields/` 通过（7 passed，其中既有 2 个 `IGNORE-UNTIL-FIX:B-21` fixture 保持 skip/pass）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:100-117` 与本任务完成条件；B-08/B-21 的 `FrontendReject` active rows 为 0，相关 negative fixture active 并通过，错误文案不含 forbidden terms。

### [DONE] P7-A3：B-15 when / 模式匹配用户面早拒

- 参考：`PLAN.md:100-117`、`audit/strategies/B-15.md`、`audit/UMB_categories/B-15.md`。
- 范围：B-15，55 entries，`FrontendReject`。
- 目标：`when` 完备性、arm 类型、enum variant、guard、payload arity 不再走 codegen 兜底。
- 必须实现：
  1. 用 `umb-audit list --bucket B-15` 锁定 55 个 ID。
  2. 在 frontend/typecheck/HIR/MIR verifier 中统一 `when` user-facing gate。
  3. 为 enum variant 缺失、unknown variant、payload arity、arm type mismatch、guard type 等负例产出稳定 diagnostic。
  4. 删除 B-15 codegen fallback 并更新对账文件。
- 验证：
  1. `cargo test -p scoopc audit:: -- --nocapture`
  2. `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  3. `cargo run -p scoop -- test tests/fixtures/umb_fix/B-15-when-pattern/`
  4. `cargo run -p scoop -- test tests/fixtures/typecheck/`
- 完成条件：B-15 active count 为 0；`when` negative fixture 全部 active 并通过；错误文案不含 forbidden terms。
- 依赖：P7-0-T02。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/typecheck/expr/stmt.rs`、`crates/scoopc/src/llvm/codegen/expr.rs`、`crates/scoopc/src/llvm/codegen/control_flow.rs`、`crates/scoopc/src/audit/umb_inventory.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/B-15.md`、`audit/UMB_categories/_overview.md`、`audit/strategies/B-15.md`、`audit/spec_coverage_matrix.md`、`tests/fixtures/umb_fix/_index.csv`、B-15 fixtures、`tests/fixtures/typecheck/when_guard_not_bool_is_error.scoop` 与 `memory/claude_plan.md`；同步 `control_flow.rs` line drift 影响的 B-01/B-05/B-06/B-22/B-36 文档行号。
  - Retired IDs：`UMB-0121`、`UMB-0122`、`UMB-0125`、`UMB-0126`、`UMB-0127`、`UMB-0128`、`UMB-0130`、`UMB-0131`、`UMB-0132`、`UMB-0133`、`UMB-0134`、`UMB-0135`、`UMB-0136`、`UMB-0137`、`UMB-0138`、`UMB-0139`、`UMB-0140`、`UMB-0141`、`UMB-0142`、`UMB-0143`、`UMB-0144`、`UMB-0146`、`UMB-0147`、`UMB-0148`、`UMB-0150`、`UMB-0151`、`UMB-0152`、`UMB-0154`、`UMB-0155`、`UMB-0156`、`UMB-0157`、`UMB-0159`、`UMB-0160`、`UMB-0161`、`UMB-0162`、`UMB-0165`、`UMB-0166`、`UMB-0167`、`UMB-0168`、`UMB-0169`、`UMB-0170`、`UMB-0171`、`UMB-0172`、`UMB-0173`、`UMB-0175`、`UMB-0176`、`UMB-0177`、`UMB-0178`、`UMB-0179`、`UMB-0180`、`UMB-0181`、`UMB-0182`、`UMB-0183`、`UMB-0184`、`UMB-0186`；数量 55；bucket `B-15`；class `FrontendReject`。
  - 核心决策：补齐 statement-position `when` guard Bool gate；保持 expression-position 既有 exhaustiveness、variant unknown/arity、tuple arity 与 arm expected-type gate；`when` codegen 在无显式 expected 时使用 HIR 结果类型，非 switch/`is` pattern 走 verified chain matcher；LLVM lowering 删除 B-15 `UnsupportedMainBody` fallback，改为上游 gate 后的 internal invariant。
  - Inventory/ledger：active 1,272 -> 1,217；retired 12 -> 67；B-15 active 55 -> 0。
  - Stale count：tracked stale baseline 未列入 `control_flow.rs`，`pipeline_user_visible_failure_policy` tracked total 保持 633 -> 633；`control_flow.rs` active inventory rows 74 -> 19。
  - Fixture 状态：B-15 fixture directory 从 `IGNORE-UNTIL-FIX:B-15` 激活；新增 `neg_when_guard_not_bool.scoop`、`neg_when_unknown_variant.scoop`、`neg_when_variant_payload_arity.scoop`、`neg_when_arm_expected_type.scoop`、`pos_when_tuple_enum_payload.scoop` 与 typecheck fixture `when_guard_not_bool_is_error.scoop`；retired IDs 改由 retired ledger 覆盖，active fixture `COVERS` 不再引用 retired IDs 或 B-07 IDs。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-15` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，1,217 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=1,217、retired=67、initial=1,284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-15-when-pattern/` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/typecheck/` 通过（493 passed）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:100-117` 与本任务完成条件；B-15 active count 为 0，`when` negative fixtures active 并通过，错误文案不含 forbidden terms。

### [DONE] P7-A4：B-36 spec-uncovered surface 早拒

- 参考：`PLAN.md:100-117`、`audit/strategies/B-36.md`、`tests/fixtures/umb_fix/B-36-spec-uncovered/`。
- 范围：B-36，58 entries，`FrontendReject`。
- 目标：async/generator/yield/class/annotation 等未定义或暂未支持 surface 产出稳定 frontend diagnostic。
- 必须实现：
  1. 用 `umb-audit list --bucket B-36` 锁定 58 个 ID。
  2. 对当前 spec 未定义 surface 建立早拒 gate，不扩展 spec 语义。
  3. 删除对应 codegen fallback，必要处保留 “blocked-on-spec” 文档说明。
  4. 更新 B-36 category/strategy、spec matrix、inventory、retired ledger、fixture index。
- 验证：
  1. `cargo test -p scoopc audit:: -- --nocapture`
  2. `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  3. `cargo run -p scoop -- test tests/fixtures/umb_fix/B-36-spec-uncovered/`
- 完成条件：B-36 active count 为 0；D 类相关 fixture active 并通过；未引入 spec 扩展。
- 依赖：P7-0-T02。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/resolve/mod.rs`、`crates/scoopc/src/resolve/scopes.rs`、多处 `crates/scoopc/src/llvm/codegen/**` B-36 fallback、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`、`crates/scoopc/src/llvm/mod.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/B-36.md`、`audit/UMB_categories/_overview.md`、`audit/strategies/B-36.md`、`audit/spec_coverage_matrix.md`、B-36 fixtures、`tests/fixtures/umb_fix/_index.csv` 与 `memory/claude_plan.md`。
  - Retired IDs：`UMB-0005`、`UMB-0008`、`UMB-0009`、`UMB-0012`、`UMB-0013`、`UMB-0014`、`UMB-0015`、`UMB-0017`、`UMB-0018`、`UMB-0097`、`UMB-0101`、`UMB-0189`、`UMB-0225`、`UMB-0239`、`UMB-0411`、`UMB-0412`、`UMB-0413`、`UMB-0418`、`UMB-0739`、`UMB-0743`、`UMB-0744`、`UMB-0747`、`UMB-0748`、`UMB-0749`、`UMB-0756`、`UMB-0759`、`UMB-0760`、`UMB-0761`、`UMB-0768`、`UMB-0769`、`UMB-0771`、`UMB-0773`、`UMB-0818`、`UMB-0819`、`UMB-0829`、`UMB-0879`、`UMB-0886`、`UMB-0893`、`UMB-0894`、`UMB-0897`、`UMB-0898`、`UMB-0902`、`UMB-0903`、`UMB-0915`、`UMB-0916`、`UMB-1176`、`UMB-1182`、`UMB-1190`、`UMB-1197`、`UMB-1213`、`UMB-1265`、`UMB-1270`、`UMB-1271`、`UMB-1273`、`UMB-1274`、`UMB-1275`、`UMB-1278`、`UMB-1279`；数量 58；bucket `B-36`；class `FrontendReject`。
  - 核心决策：新增 resolve 级 `async`/`await` 与 generator/`yield` spec-uncovered surface diagnostic；不扩展 spec 语义；LLVM B-36 `UnsupportedMainBody` fallback 改为上游 gate 后的 internal invariant/expectation。
  - Inventory/ledger：active 1,217 -> 1,159；retired 67 -> 125；B-36 active 58 -> 0；`FrontendReject` active 58 -> 0。
  - Stale count：tracked stale total 633 -> 610；`effect_lowered/value.rs` 139 -> 137；`main/alloca.rs` 7 -> 6；`main/boxing.rs` 6 -> 3；`main/context.rs` 4 -> 2；`main/declare.rs` 8 -> 7；`main/frame.rs` 12 -> 10；`main/gc_locals.rs` 11 -> 5；`main/identity.rs` 6 -> 4；`mir_body/mod.rs` 6 -> 5；`mir_body/operand.rs` 8 -> 7；`mir_body/terminator.rs` 19 -> 18；`mir_body/transport.rs` 10 -> 9。
  - Fixture 状态：B-36 fixture directory 从 `IGNORE-UNTIL-FIX:B-36` 激活；async/await、generator/yield、spec-meta placeholder negative fixtures 使用 frontend resolve diagnostics；annotation fixtures active 但 `COVERS: NONE`；retired IDs 改由 retired ledger 覆盖，active fixture `COVERS` 不再引用 retired IDs。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-36` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，1,159 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=1,159、retired=125、initial=1,284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-36-spec-uncovered/` 通过（6 passed）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:100-117` 与本任务完成条件；B-36 active count 为 0，D 类相关 fixtures active 并通过，未引入 async/generator/yield 等 spec 扩展。

## P7-B：InternalBugSentinel 退场（956 entries）

### [DONE] P7-B1：B-01 helper invariant 统一迁移

- 参考：`PLAN.md:123-133`、`audit/strategies/B-01.md`。
- 范围：B-01，71 entries，`InternalBugSentinel`。
- 目标：统一 LLVM insertion context helper，删除分散的 B-01 `UnsupportedMainBody` fallback。
- 必须实现：
  1. 引入 `expect_insert_block`、`expect_parent_function`、`expect_entry_block`、`expect_basic_value` 或等价集中 helper。
  2. helper panic 文案包含 helper 名称和上下文，不复用 UMB diagnostic。
  3. 迁移 B-01 所有 builder/current-function/entry-block/basic-value 读取点。
  4. 将 B-01 sentinel coverage 从 README/audit test 迁入 retired ledger。
  5. 更新 B-01 文档、inventory、fixture coverage、stale count。
- 验证：
  1. `cargo run -p scoopc --bin umb-audit -- list --bucket B-01`
  2. `cargo test -p scoopc audit:: -- --nocapture`
  3. `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  4. `cargo test -p scoopc llvm -- --nocapture`
- 完成条件：B-01 active count 为 0；B-01 sentinel test 仍证明 helper-only 覆盖闭环。
- 依赖：P7-0-T02。
- 完成记录：
  - 改动范围：新增集中 LLVM insertion/value invariant helper 于 `crates/scoopc/src/llvm/codegen/main/context.rs`；迁移 B-01 fallback 所在的 `crates/scoopc/src/llvm/codegen/{call/lowering.rs,class_ctor.rs,closure/mod.rs,control_flow.rs,effect_lowered/value.rs,gc.rs,intrinsics/atomic.rs,intrinsics/named.rs,main/alloca.rs,main/expr_op.rs,main/frame.rs,main/immut_value.rs,main/numeric.rs,mir_body/args.rs,mir_body/callable_lookup.rs,mir_body/cast.rs,mir_body/const_pat.rs,mir_body/member.rs,stmt.rs}`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/B-01.md`、`audit/UMB_categories/_overview.md`、`audit/strategies/B-01.md`、`audit/spec_coverage_matrix.md`、`tests/fixtures/umb_fix/B-01-builder-invariant/_README.md`、`crates/scoopc/src/audit/{umb_inventory.rs,spec_coverage.rs,sentinel_tests.rs}`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`、`crates/scoopc/src/llvm/codegen/mir_body/mod.rs` 与 `memory/claude_plan.md`。
  - Retired IDs：`UMB-0020`、`UMB-0064`、`UMB-0093`、`UMB-0118`、`UMB-0119`、`UMB-0123`、`UMB-0124`、`UMB-0163`、`UMB-0164`、`UMB-0296`、`UMB-0297`、`UMB-0344`、`UMB-0345`、`UMB-0352`、`UMB-0353`、`UMB-0433`、`UMB-0434`、`UMB-0444`、`UMB-0445`、`UMB-0456`、`UMB-0457`、`UMB-0468`、`UMB-0469`、`UMB-0479`、`UMB-0480`、`UMB-0541`、`UMB-0542`、`UMB-0607`、`UMB-0609`、`UMB-0762`、`UMB-0763`、`UMB-0764`、`UMB-0765`、`UMB-0766`、`UMB-0767`、`UMB-0838`、`UMB-0839`、`UMB-0840`、`UMB-0848`、`UMB-0849`、`UMB-0852`、`UMB-0853`、`UMB-0854`、`UMB-0857`、`UMB-0858`、`UMB-0878`、`UMB-0880`、`UMB-0888`、`UMB-0889`、`UMB-0925`、`UMB-0926`、`UMB-0943`、`UMB-0944`、`UMB-0985`、`UMB-1025`、`UMB-1065`、`UMB-1066`、`UMB-1067`、`UMB-1068`、`UMB-1069`、`UMB-1101`、`UMB-1102`、`UMB-1155`、`UMB-1156`、`UMB-1165`、`UMB-1166`、`UMB-1174`、`UMB-1175`、`UMB-1266`、`UMB-1267`、`UMB-1269`；数量 71；bucket `B-01`；class `InternalBugSentinel`。
  - 核心决策：B-01 为内部 helper invariant，不新增用户 `.scoop` fixture；`expect_insert_block`、`expect_parent_function`、`expect_current_function`、`expect_entry_block`、`expect_basic_value` 和 explicit-frame instruction-parent helper 统一承接 panic boundary，panic 文案包含 helper 名称和上下文且不复用 UMB diagnostic；同步修正一个 stale LLVM 单测，使非 Unit empty MIR return contract 断言 internal panic 而不是旧 UMB error。
  - Inventory/ledger：active 1,159 -> 1,088；retired 125 -> 196；B-01 active 71 -> 0；`InternalBugSentinel` active 956 -> 885。
  - Stale count：`effect_lowered/value.rs` 137 -> 131；`main/alloca.rs` 6 -> 0；`main/expr_op.rs` 29 -> 19；`main/frame.rs` 10 -> 6；`main/immut_value.rs` 18 -> 16；`main/numeric.rs` 2 -> 0；`mir_body/args.rs` 15 -> 14；`mir_body/callable_lookup.rs` 21 -> 20；`mir_body/cast.rs` 28 -> 23；`mir_body/const_pat.rs` 38 -> 36；`mir_body/member.rs` 48 -> 42；tracked stale total 610 -> 565。
  - Fixture 状态：B-01 继续 README/sentinel-only；`tests/fixtures/umb_fix/B-01-builder-invariant/_README.md` 状态改为 `retired-by-P7-B1`，`SENTINEL-COVERS` 仍列出 71 个 retired helper-invariant IDs；fixture coverage audit 不再把 B-01 sentinel IDs 计入 active inventory coverage。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-01` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，1,088 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=1,088、retired=196、initial=1,284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo test -p scoopc llvm -- --nocapture` 通过（250 passed）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:123-133` 与本任务完成条件；B-01 active count 为 0，B-01 sentinel test 仍证明 helper-only coverage 已由 retired ledger 覆盖。

### [DONE] P7-B2.1：B-02/B-04 MIR local、param、return type contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-02.md`、`audit/strategies/B-04.md`。
- 范围：B-02 6 entries；B-04 29 entries；合计 35 entries。
- 目标：local、param、return type 在 MIR materialize/strict verifier 后完整，不由 codegen 兜底。
- 必须实现：MIR 类型完整性 verifier、param/return arity 和 type gate、codegen fallback retire、inventory/ledger/stale count 对账。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-02-mir-local-type/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-04-function-signature/`。
- 完成条件：B-02/B-04 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/lower/entry.rs`、`crates/scoopc/src/mir/materialize/validation.rs`、`crates/scoopc/src/mir/materialize/tests.rs`、`crates/scoopc/src/pipeline/mir_stage.rs`；迁移 B-02/B-04 fallback 所在的 `crates/scoopc/src/llvm/codegen/{call/abi.rs,closure/mod.rs,main/{boxing.rs,context.rs,declare.rs,expr_value.rs,function.rs,gc_locals.rs,immut_value.rs,literal.rs},mir_body/{args.rs,dispatch.rs,string.rs,terminator.rs,transport.rs,types.rs}}`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-02.md,B-04.md,_overview.md}`、`audit/strategies/{B-02.md,B-04.md}`、`audit/spec_coverage_matrix.md`、`tests/fixtures/umb_fix/_index.csv`、B-02/B-04 fixtures 与 `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`；同步记录 `memory/claude_plan.md`。
  - Retired IDs：`UMB-0859`、`UMB-0899`、`UMB-0921`、`UMB-1191`、`UMB-1221`、`UMB-1222`、`UMB-0001`、`UMB-0002`、`UMB-0003`、`UMB-0004`、`UMB-0098`、`UMB-0102`、`UMB-0770`、`UMB-0772`、`UMB-0774`、`UMB-0822`、`UMB-0823`、`UMB-0824`、`UMB-0825`、`UMB-0826`、`UMB-0890`、`UMB-0891`、`UMB-0892`、`UMB-0939`、`UMB-0942`、`UMB-0980`、`UMB-0981`、`UMB-0983`、`UMB-0984`、`UMB-0994`、`UMB-1112`、`UMB-1119`、`UMB-1192`、`UMB-1194`、`UMB-1214`；数量 35；bucket `B-02`/`B-04`；class `InternalBugSentinel`。
  - 核心决策：MIR production/materialized verifier 统一校验 local reference、assignment target、parameter local/type、return operand presence、member/store operand type、constructor ordered-arg shape；LLVM lowering 删除 B-02/B-04 `UnsupportedMainBody` fallback，改为 verifier 后的 internal panic/`expect_*` invariant。
  - Inventory/ledger：active 1,088 -> 1,053；retired 196 -> 231；B-02 active 6 -> 0；B-04 active 29 -> 0；`InternalBugSentinel` active 885 -> 850。
  - Stale count：tracked stale total 565 -> 536；`mir_body/args.rs` 14 -> 9；`mir_body/dispatch.rs` 16 -> 14；`mir_body/string.rs` 1 -> 0；`mir_body/terminator.rs` 18 -> 16；`mir_body/transport.rs` 9 -> 8；`mir_body/types.rs` 4 -> 2；`main/boxing.rs` 3 -> 0；`main/declare.rs` 7 -> 2；`main/expr_value.rs` 18 -> 17；`main/function.rs` 3 -> 0；`main/gc_locals.rs` 5 -> 4；`main/immut_value.rs` 16 -> 15；`main/literal.rs` 4 -> 2。
  - Fixture 状态：B-02/B-04 fixture directories 从 `IGNORE-UNTIL-FIX` 激活；retired IDs 改由 retired ledger 覆盖，active fixture `COVERS` 不再引用 B-02/B-04 retired IDs；跨 bucket fixture coverage 保留仍 active 的 B-03/B-09 IDs。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-02` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- list --bucket B-04` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=1,053、retired=231、initial=1,284）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，1,053 entries）；`cargo test -p scoopc mir:: -- --nocapture` 通过（106 passed）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-02-mir-local-type/` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-04-function-signature/` 通过（3 passed）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:135-159` 与本任务完成条件；B-02/B-04 active count 为 0，local/param/return type contract 不再由 LLVM codegen 兜底。

### [DONE] P7-B2.2：B-05 MIR CFG contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-05.md`。
- 范围：B-05，25 entries。
- 目标：CFG start block、goto/branch target、terminator shape 在 MIR verifier 后合法。
- 必须实现：CFG verifier gate、target existence/arity check、terminator type check、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-05-mir-cfg/`。
- 完成条件：B-05 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/materialize/validation.rs`、`crates/scoopc/src/mir/materialize/tests.rs`、`crates/scoopc/src/pipeline/mir_stage.rs`；迁移 B-05 fallback 所在的 `crates/scoopc/src/llvm/codegen/control_flow.rs`、`crates/scoopc/src/llvm/codegen/mir_body/dispatch.rs`、`crates/scoopc/src/llvm/codegen/mir_body/terminator.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-05.md,B-06.md,B-10.md,B-12.md,B-20.md,B-22.md,B-23.md,B-35.md,_overview.md}`、`audit/strategies/B-05.md`、`audit/spec_coverage_matrix.md`、B-05/B-16 fixtures、`tests/fixtures/umb_fix/_index.csv`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 与 `memory/claude_plan.md`。
  - Retired IDs：`UMB-0116`、`UMB-0117`、`UMB-0120`、`UMB-0190`、`UMB-0193`、`UMB-1110`、`UMB-1114`、`UMB-1115`、`UMB-1117`、`UMB-1121`、`UMB-1122`、`UMB-1125`、`UMB-1193`、`UMB-1195`、`UMB-1196`、`UMB-1198`、`UMB-1199`、`UMB-1200`、`UMB-1201`、`UMB-1202`、`UMB-1204`、`UMB-1205`、`UMB-1206`、`UMB-1208`、`UMB-1210`；数量 25；bucket `B-05`；class `InternalBugSentinel`。
  - 核心决策：MIR production/materialized verifier 统一校验 CFG/direct-style 形状、`CondBr` Bool 条件、param/local bounds、residual interpolated-string rvalue 和 Todo rvalue；LLVM lowering 删除 B-05 `UnsupportedMainBody` fallback，改为 verifier 后的 internal invariant。
  - Inventory/ledger：active 1,053 -> 1,028；retired 231 -> 256；B-05 active 25 -> 0；`InternalBugSentinel` active 850 -> 825。
  - Stale count：tracked stale total 536 -> 516；`mir_body/dispatch.rs` 14 -> 7；`mir_body/terminator.rs` 16 -> 3；`control_flow.rs` active inventory rows 12 -> 7（不在 tracked stale list）。
  - Fixture 状态：B-05 fixture directory 从 `IGNORE-UNTIL-FIX:B-05` 激活；retired IDs 改由 retired ledger 覆盖，active fixture `COVERS` 不再引用 B-05 retired IDs；B-16 cross-coverage 中的 B-05 retired IDs 已清理。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-05` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，1,028 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=1,028、retired=256、initial=1,284）；`cargo test -p scoopc mir:: -- --nocapture` 通过（110 passed）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-05-mir-cfg/` 通过（4 passed）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:135-159` 与本任务完成条件；B-05 active count 为 0，CFG/branch/terminator contract 不再由 LLVM codegen 兜底。

### [DONE] P7-B2.3：B-06/B-07/B-21 aggregate、pattern、field schema contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-06.md`、`audit/strategies/B-07.md`、`audit/strategies/B-21.md`。
- 范围：B-06 43 entries；B-07 34 entries；B-21 internal rows 3 entries；合计 80 entries。
- 目标：struct/tuple/enum literal、pattern clause、field schema 在 MIR verifier 后闭合。
- 必须实现：aggregate schema verifier、pattern arity/type verifier、field index/name verifier、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-06-literal-schema/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-07-pattern-schema/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-21-struct-fields/`。
- 完成条件：B-06/B-07/B-21 internal rows active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/mir/materialize/validation.rs`、`crates/scoopc/src/mir/materialize/tests.rs`；迁移 B-06/B-07/B-21 fallback 所在的 `crates/scoopc/src/llvm/codegen/{call/abi.rs,control_flow.rs,layout.rs,main/{expr_value.rs,frame.rs},mir_body/{aggregates.rs,const_pat.rs,member.rs},ty.rs}`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-06.md,B-07.md,B-21.md,_overview.md}`、`audit/strategies/{B-06.md,B-07.md,B-21.md}`、`audit/spec_coverage_matrix.md`、B-06/B-07/B-21 fixtures、`tests/fixtures/umb_fix/_index.csv`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 与 `memory/claude_plan.md`。
  - Retired IDs：B-06 `UMB-0010`、`UMB-0145`、`UMB-0174`、`UMB-0185`、`UMB-0752`、`UMB-0753`、`UMB-0754`、`UMB-0755`、`UMB-0860`、`UMB-0861`、`UMB-0862`、`UMB-0865`、`UMB-0866`、`UMB-0867`、`UMB-0868`、`UMB-0869`、`UMB-0883`、`UMB-0884`、`UMB-0949`、`UMB-0950`、`UMB-0951`、`UMB-0952`、`UMB-0953`、`UMB-0955`、`UMB-0958`、`UMB-0959`、`UMB-0960`、`UMB-0961`、`UMB-0963`、`UMB-0964`、`UMB-0965`、`UMB-0966`、`UMB-0967`、`UMB-0968`、`UMB-0970`、`UMB-0972`、`UMB-0973`、`UMB-0974`、`UMB-0975`、`UMB-1281`、`UMB-1282`、`UMB-1283`、`UMB-1284`；B-07 `UMB-1072`、`UMB-1073`、`UMB-1074`、`UMB-1075`、`UMB-1076`、`UMB-1077`、`UMB-1078`、`UMB-1079`、`UMB-1080`、`UMB-1081`、`UMB-1082`、`UMB-1083`、`UMB-1084`、`UMB-1085`、`UMB-1087`、`UMB-1088`、`UMB-1090`、`UMB-1091`、`UMB-1092`、`UMB-1093`、`UMB-1094`、`UMB-1095`、`UMB-1096`、`UMB-1097`、`UMB-1098`、`UMB-1099`、`UMB-1100`、`UMB-1103`、`UMB-1104`、`UMB-1105`、`UMB-1106`、`UMB-1107`、`UMB-1108`、`UMB-1109`；B-21 `UMB-0864`、`UMB-1141`、`UMB-1272`；数量 80；bucket `B-06`/`B-07`/`B-21`；class `InternalBugSentinel`。
  - 核心决策：materialized MIR verifier 统一校验 aggregate transport kind/type/arity/field name/field type/value transport、pattern subject/literal/tuple/variant schema、pattern extract path/result type，以及 value member target metadata；LLVM lowering 删除对应 `UnsupportedMainBody` fallback，改为 verifier 后的 internal invariant panic/`expect`/`unreachable!`。
  - Inventory/ledger：active 1,028 -> 948；retired 256 -> 336；B-06 active 43 -> 0；B-07 active 34 -> 0；B-21 active 3 -> 0；`InternalBugSentinel` active 825 -> 745。
  - Stale count：tracked stale total 516 -> 449；`mir_body/aggregates.rs` 30 -> 9；`mir_body/const_pat.rs` 36 -> 2；`mir_body/member.rs` 42 -> 41；`main/expr_value.rs` 17 -> 8；`main/frame.rs` 6 -> 4；`call/abi.rs`、`control_flow.rs`、`layout.rs`、`ty.rs` 另删除目标 inventory rows（不在 tracked stale list）。
  - Fixture 状态：B-06/B-07 fixture directories 从 `IGNORE-UNTIL-FIX` 激活；B-21 active fixtures 移除 retired B-21 COVERS；retired IDs 改由 retired ledger 覆盖，active fixture `COVERS` 不再引用 retired B-06/B-07/B-21 IDs。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-06` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- list --bucket B-07` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- list --bucket B-21` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，948 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=948、retired=336、initial=1,284）；`cargo test -p scoopc mir::materialize -- --nocapture` 通过（43 passed）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-06-literal-schema/` 通过（4 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-07-pattern-schema/` 通过（2 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-21-struct-fields/` 通过（7 passed，其中既有 2 个 `IGNORE-UNTIL-FIX:B-21` fixture 保持 skip/pass）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:135-159` 与本任务完成条件；B-06/B-07/B-21 internal rows active count 为 0，aggregate/pattern/field schema contract 不再由 LLVM codegen 兜底。

### [DONE] P7-B2.4：B-03/B-09/B-14 call ABI、TypeStore、cast contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-03.md`、`audit/strategies/B-09.md`、`audit/strategies/B-14.md`。
- 范围：B-03 56 entries；B-09 13 entries；B-14 27 entries；合计 96 entries。
- 目标：direct/closure/funptr call ABI、cross-TypeStore equivalence、`as`/`as?`/`is` contract 闭合。
- 必须实现：集中 TypeStore equivalence helper、call ABI verifier、cast/typecheck verifier、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-03-call-abi/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-09-type-equivalence/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-14-cast-typecheck/`。
- 完成条件：B-03/B-09/B-14 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/mir/materialize/validation.rs`、`crates/scoopc/src/mir/materialize/tests.rs`；迁移 B-03/B-09/B-14 fallback 所在的 `crates/scoopc/src/llvm/codegen/{call/{abi.rs,lowering.rs},effect_lowered/body/call_invoke.rs,effect_outcome.rs,intrinsics/{named.rs,sysroot.rs},main/{expr_op.rs,gc_locals.rs,runtime_error.rs},mir_body/{aggregates.rs,args.rs,call.rs,callable_lookup.rs,cast.rs,operand.rs},ordinary_callee.rs}`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-03.md,B-09.md,B-14.md,_overview.md}`、`audit/strategies/{B-03.md,B-09.md,B-14.md}`、B-03/B-09/B-14 fixtures、`tests/fixtures/umb_fix/_index.csv`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 与 `memory/claude_plan.md`。
  - Retired IDs：B-03 `UMB-0011`、`UMB-0016`、`UMB-0019`、`UMB-0023`、`UMB-0024`、`UMB-0025`、`UMB-0027`、`UMB-0028`、`UMB-0030`、`UMB-0032`、`UMB-0041`、`UMB-0042`、`UMB-0043`、`UMB-0044`、`UMB-0045`、`UMB-0046`、`UMB-0047`、`UMB-0048`、`UMB-0050`、`UMB-0053`、`UMB-0054`、`UMB-0055`、`UMB-0056`、`UMB-0057`、`UMB-0058`、`UMB-0059`、`UMB-0988`、`UMB-0989`、`UMB-0990`、`UMB-0991`、`UMB-0992`、`UMB-0993`、`UMB-0996`、`UMB-0998`、`UMB-0999`、`UMB-1000`、`UMB-1001`、`UMB-1002`、`UMB-1003`、`UMB-1006`、`UMB-1010`、`UMB-1011`、`UMB-1012`、`UMB-1016`、`UMB-1021`、`UMB-1022`、`UMB-1026`、`UMB-1029`、`UMB-1030`、`UMB-1031`、`UMB-1033`、`UMB-1034`、`UMB-1038`、`UMB-1039`、`UMB-1040`、`UMB-1244`；B-09 `UMB-0035`、`UMB-0039`、`UMB-0195`、`UMB-0377`、`UMB-0378`、`UMB-0604`、`UMB-0718`、`UMB-0948`、`UMB-0969`、`UMB-1050`、`UMB-1055`、`UMB-1058`、`UMB-1183`；B-14 `UMB-0830`、`UMB-0831`、`UMB-0832`、`UMB-0842`、`UMB-0844`、`UMB-0845`、`UMB-0846`、`UMB-0847`、`UMB-0850`、`UMB-0851`、`UMB-0855`、`UMB-0856`、`UMB-0895`、`UMB-0896`、`UMB-1045`、`UMB-1046`、`UMB-1048`、`UMB-1049`、`UMB-1051`、`UMB-1052`、`UMB-1053`、`UMB-1054`、`UMB-1056`、`UMB-1059`、`UMB-1064`、`UMB-1070`、`UMB-1071`；数量 96；bucket `B-03`/`B-09`/`B-14`；class `InternalBugSentinel`。
  - 核心决策：materialized MIR verifier 统一校验 direct/function-value/FunPtr call binding、callee form、return transport、runtime type-test metadata、cast failure/result contract 和 `as?` Option result shape；TypeStore/codegen equivalence fallback 改为 verifier-backed invariant；B-25/B-34/B-17 等非本任务 rows 保持 active，不顺手退场。
  - Inventory/ledger：active 948 -> 852；retired 336 -> 432；B-03/B-09/B-14 active count 均为 0；`InternalBugSentinel` active 745 -> 649。
  - Stale count：tracked stale total 449 -> 386；`mir_body/aggregates.rs` 9 -> 8；`mir_body/args.rs` 9 -> 3；`mir_body/call.rs` 28 -> 14；`mir_body/callable_lookup.rs` 20 -> 11；`mir_body/cast.rs` 23 -> 7；`mir_body/operand.rs` 7 -> 6；`effect_lowered/body/call_invoke.rs` 4 -> 3；`main/expr_op.rs` 19 -> 7；`main/gc_locals.rs` 4 -> 2；`main/runtime_error.rs` 4 -> 3。
  - Fixture 状态：B-03/B-09/B-14 fixture directories 从 `IGNORE-UNTIL-FIX` 激活；retired IDs 改由 retired ledger 覆盖，相关 active fixture `COVERS` 不再引用 retired B-03/B-09/B-14 IDs；B-02/B-04 cross-coverage 中的 B-03/B-09 retired IDs 已清理。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-03`、`--bucket B-09`、`--bucket B-14` 均通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，852 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=852、retired=432、initial=1,284）；`cargo test -p scoopc mir::materialize -- --nocapture` 通过（46 passed）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-03-call-abi/` 通过（3 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-09-type-equivalence/` 通过（5 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-14-cast-typecheck/` 通过（2 passed）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:135-159` 与本任务完成条件；B-03/B-09/B-14 active count 为 0，call ABI、cross-TypeStore equivalence、cast/typecheck contract 不再由 LLVM codegen 兜底。

### [DONE] P7-B2.5：B-17/B-18 scalar coercion 与 literal/string contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-17.md`、`audit/strategies/B-18.md`。
- 范围：B-17 47 entries；B-18 4 entries；合计 51 entries。
- 目标：coercion、equality、bool/string operator、literal slice/value contract 不再由 codegen 兜底。
- 必须实现：标量 coercion helper、literal source/value contract、string equality/load contract、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-17-coercion-scalar/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-18-literals-strings/`。
- 完成条件：B-17/B-18 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/codegen/{effect_outcome.rs,expr.rs,main/{coerce.rs,context.rs,expr_op.rs,literal.rs}}`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-10.md,B-13.md,B-17.md,B-18.md,B-29.md,_overview.md}`、`audit/strategies/{B-17.md,B-18.md}`、`audit/spec_coverage_matrix.md`、`tests/fixtures/umb_fix/_index.csv`、B-17/B-18/B-31 fixtures、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 与 `memory/claude_plan.md`。
  - Retired IDs：B-17 `UMB-0367`、`UMB-0368`、`UMB-0369`、`UMB-0370`、`UMB-0373`、`UMB-0414`、`UMB-0415`、`UMB-0416`、`UMB-0417`、`UMB-0787`、`UMB-0788`、`UMB-0789`、`UMB-0790`、`UMB-0791`、`UMB-0792`、`UMB-0793`、`UMB-0794`、`UMB-0795`、`UMB-0796`、`UMB-0797`、`UMB-0798`、`UMB-0799`、`UMB-0800`、`UMB-0801`、`UMB-0802`、`UMB-0803`、`UMB-0804`、`UMB-0805`、`UMB-0806`、`UMB-0807`、`UMB-0808`、`UMB-0809`、`UMB-0810`、`UMB-0811`、`UMB-0812`、`UMB-0813`、`UMB-0814`、`UMB-0815`、`UMB-0816`、`UMB-0817`、`UMB-0833`、`UMB-0834`、`UMB-0835`、`UMB-0836`、`UMB-0837`、`UMB-0841`、`UMB-0843`；B-18 `UMB-0820`、`UMB-0821`、`UMB-0940`、`UMB-0941`；数量 51；bucket `B-17`/`B-18`；class `InternalBugSentinel`。
  - 核心决策：集中新增 `expect_cg_*`/`expect_int_value` helper 承接 scalar payload、string equality return、literal allocation return 的 internal invariant；raw HIR operator/cast fallback 改为 typecheck/HIR lowering gate 后的 panic boundary；保留 B-13/B-29 非本任务 effect transport rows active，不顺手退场。
  - Inventory/ledger：active 852 -> 801；retired 432 -> 483；B-17 active 47 -> 0；B-18 active 4 -> 0；`InternalBugSentinel` active 649 -> 598。
  - Stale count：tracked stale total 386 -> 344；`main/coerce.rs` 31 -> 0；`main/context.rs` 2 -> 0；`main/expr_op.rs` 7 -> 0；`main/literal.rs` 2 -> 0；`effect_outcome.rs`/`expr.rs` 删除目标 inventory rows 但不在 tracked stale list。
  - Fixture 状态：B-17/B-18 fixture directories 从 `IGNORE-UNTIL-FIX` 激活；active fixture `COVERS` 不再引用 retired B-17/B-18 IDs；B-31 pending fixture 仅保留仍 active 的 B-31 IDs；B-18 lexical positive fixture补齐 type annotation 以满足 full fixture pipeline。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-17` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- list --bucket B-18` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，801 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=801、retired=483、initial=1,284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-17-coercion-scalar/` 通过（5 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-18-literals-strings/` 通过（4 passed）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:135-159` 与本任务完成条件；B-17/B-18 active count 为 0，scalar coercion/operator、literal/string contract 不再由 LLVM codegen 兜底。

### [DONE] P7-B2.6：B-19/B-20/B-22/B-23 layout 与 member contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-19.md`、`audit/strategies/B-20.md`、`audit/strategies/B-22.md`、`audit/strategies/B-23.md`。
- 范围：B-19 39 entries；B-20 46 entries；B-22 40 entries；B-23 24 entries；合计 149 entries。
- 目标：top-level/object/class/enum/member layout 和 access contract 在 codegen 前闭合。
- 必须实现：top-level metadata gate、object/class ctor/member layout verifier、enum/niche layout verifier、member receiver/target verifier、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-19-top-level-object-extern/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-20-class-property-field/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-22-enum-niche-option/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-23-member-access/`。
- 完成条件：B-19/B-20/B-22/B-23 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/mir/materialize/validation.rs` 与 `tests.rs`，迁移 B-19/B-20/B-22/B-23 fallback 所在的 `crates/scoopc/src/llvm/codegen/{call/lowering.rs,class_ctor.rs,control_flow.rs,enum_lowering.rs,layout.rs,main/{call.rs,expr_value.rs,frame.rs,globals.rs,immut_value.rs},mir_body/{args.rs,call.rs,dispatch.rs,member.rs,mod.rs,types.rs},object_init.rs,ty.rs}`；同步 `crates/scoopc/src/{audit/umb_inventory.rs,pipeline_user_visible_failure_policy.rs}`、`audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-19.md,B-20.md,B-22.md,B-23.md,_overview.md}`、`audit/strategies/{B-19.md,B-20.md,B-22.md,B-23.md}`、相关 B-19/B-20/B-22/B-23 fixtures、`tests/fixtures/umb_fix/_index.csv` 与 `memory/claude_plan.md`。
  - Retired IDs：B-19 `UMB-0036`、`UMB-0887`、`UMB-0904`、`UMB-0905`、`UMB-0906`、`UMB-0907`、`UMB-0910`、`UMB-0911`、`UMB-0912`、`UMB-0913`、`UMB-0914`、`UMB-0922`、`UMB-0923`、`UMB-0924`、`UMB-0927`、`UMB-0928`、`UMB-0929`、`UMB-0930`、`UMB-0931`、`UMB-0932`、`UMB-0933`、`UMB-0934`、`UMB-0935`、`UMB-0936`、`UMB-1138`、`UMB-1139`、`UMB-1231`、`UMB-1232`、`UMB-1233`、`UMB-1234`、`UMB-1235`、`UMB-1236`、`UMB-1237`、`UMB-1238`、`UMB-1239`、`UMB-1240`、`UMB-1241`、`UMB-1242`、`UMB-1243`；B-20 `UMB-0061`、`UMB-0062`、`UMB-0063`、`UMB-0065`、`UMB-0066`、`UMB-0067`、`UMB-0068`、`UMB-0069`、`UMB-0070`、`UMB-0071`、`UMB-0072`、`UMB-0073`、`UMB-0074`、`UMB-0075`、`UMB-0076`、`UMB-0077`、`UMB-0078`、`UMB-0079`、`UMB-0080`、`UMB-0081`、`UMB-0082`、`UMB-0083`、`UMB-0084`、`UMB-0085`、`UMB-0086`、`UMB-0087`、`UMB-0088`、`UMB-0089`、`UMB-0090`、`UMB-0091`、`UMB-0745`、`UMB-0746`、`UMB-0751`、`UMB-0781`、`UMB-0782`、`UMB-0783`、`UMB-0870`、`UMB-0871`、`UMB-0982`、`UMB-0986`、`UMB-0987`、`UMB-1004`、`UMB-1123`、`UMB-1124`、`UMB-1276`、`UMB-1277`；B-22 `UMB-0040`、`UMB-0129`、`UMB-0149`、`UMB-0153`、`UMB-0158`、`UMB-0382`、`UMB-0383`、`UMB-0384`、`UMB-0385`、`UMB-0386`、`UMB-0387`、`UMB-0388`、`UMB-0389`、`UMB-0390`、`UMB-0391`、`UMB-0392`、`UMB-0393`、`UMB-0394`、`UMB-0395`、`UMB-0396`、`UMB-0397`、`UMB-0398`、`UMB-0399`、`UMB-0400`、`UMB-0401`、`UMB-0402`、`UMB-0403`、`UMB-0404`、`UMB-0405`、`UMB-0406`、`UMB-0407`、`UMB-0408`、`UMB-0409`、`UMB-0410`、`UMB-0757`、`UMB-0758`、`UMB-1127`、`UMB-1128`、`UMB-1129`、`UMB-1280`；B-23 `UMB-0021`、`UMB-0026`、`UMB-0049`、`UMB-0060`、`UMB-0872`、`UMB-0873`、`UMB-0874`、`UMB-0875`、`UMB-0876`、`UMB-0877`、`UMB-1113`、`UMB-1118`、`UMB-1120`、`UMB-1126`、`UMB-1130`、`UMB-1140`、`UMB-1143`、`UMB-1144`、`UMB-1145`、`UMB-1147`、`UMB-1177`、`UMB-1178`、`UMB-1223`、`UMB-1224`；数量 149；bucket `B-19`/`B-20`/`B-22`/`B-23`；class `InternalBugSentinel`。
  - 核心决策：materialized MIR verifier 统一校验 class ctor selected/ordered args、top-level store target/type、enum payload schema、member receiver/target、dispatch metadata；LLVM lowering 中对应 `UnsupportedMainBody` fallback 改为 verifier 后的 internal invariant panic/expect helper；generic owner-specialized member paths在 metadata 不落入当前 materialized file 时用 receiver/target owner contract 校验。
  - Inventory/ledger：active 801 -> 652；retired 483 -> 632；B-19/B-20/B-22/B-23 active count 均为 0；`InternalBugSentinel` active 598 -> 449。
  - Stale count：tracked stale total 344 -> 285；`mir_body/args.rs` 3 -> 0；`mir_body/call.rs` 14 -> 13；`mir_body/dispatch.rs` 7 -> 2；`mir_body/member.rs` 41 -> 29；`mir_body/mod.rs` 5 -> 3；`mir_body/types.rs` 2 -> 0；`main/call.rs` 11 -> 8；`main/expr_value.rs` 8 -> 0；`main/frame.rs` 4 -> 3；`main/globals.rs` 11 -> 2；`main/immut_value.rs` 15 -> 2；`class_ctor.rs`、`enum_lowering.rs`、`layout.rs`、`object_init.rs`、`ty.rs` 删除目标 inventory rows 但不在 tracked stale list。
  - Fixture 状态：B-19/B-20/B-22/B-23 fixture directories 激活并通过；retired IDs 改由 retired ledger 覆盖，active fixture `COVERS` 不再引用 retired IDs；B-19 cone missing-entry fixture保留为 plain fixture-runner package-boundary smoke，multi-file missing-entry diagnostic 仍由源 fixture 覆盖。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-19/B-20/B-22/B-23` 均通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，652 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=652、retired=632、initial=1,284）；`cargo test -p scoopc mir::materialize -- --nocapture` 通过（49 passed）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-19-top-level-object-extern/` 通过（8 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-20-class-property-field/` 通过（4 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-22-enum-niche-option/` 通过（4 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-23-member-access/` 通过（2 passed）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:135-159` 与本任务完成条件；B-19/B-20/B-22/B-23 active count 为 0，layout/member/enum/top-level contracts 不再由 LLVM `UnsupportedMainBody` 兜底。

### [DONE] P7-B2.7：B-33/B-34/B-35 extern、RuntimeError、NoGC/frame boundary contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-33.md`、`audit/strategies/B-34.md`、`audit/strategies/B-35.md`。
- 范围：B-33 8 entries；B-34 6 entries；B-35 5 entries；合计 19 entries。
- 目标：extern global、RuntimeError、try/catch/finally、NoGC/frame slot contract 不再由 codegen 兜底。
- 必须实现：extern global type/store gate、RuntimeError layout verifier、frame/spill slot consistency verifier、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-33-extern-global-funptr/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-34-runtime-error-try/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-35-unsafe-nogc-boundary/`。
- 完成条件：B-33/B-34/B-35 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/mir/materialize/validation.rs` 与 `tests.rs`；迁移 B-33/B-34/B-35 fallback 所在的 `crates/scoopc/src/llvm/codegen/{effect_lowered/body/{payload.rs,runtime_error.rs},main/{declare.rs,frame.rs,globals.rs,immut_value.rs,runtime_error.rs},mir_body/{cast.rs,member.rs,terminator.rs}}`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-33.md,B-34.md,B-35.md,_overview.md}`、`audit/strategies/{B-33.md,B-34.md,B-35.md}`、`audit/spec_coverage_matrix.md`、B-33/B-34/B-35 fixtures、`tests/fixtures/umb_fix/_index.csv`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 与 `memory/claude_plan.md`。
  - Retired IDs：B-33 `UMB-0908`、`UMB-0909`、`UMB-0937`、`UMB-0938`、`UMB-1134`、`UMB-1135`、`UMB-1136`、`UMB-1137`；B-34 `UMB-0215`、`UMB-0216`、`UMB-0217`、`UMB-0218`、`UMB-0947`、`UMB-1047`；B-35 `UMB-0827`、`UMB-0828`、`UMB-0881`、`UMB-0885`、`UMB-1209`；数量 19；bucket `B-33`/`B-34`/`B-35`；class `InternalBugSentinel`。
  - 核心决策：materialized MIR verifier 校验 extern global initializer absence、store mutability 和 type contract；RuntimeError completion payload、enum/unit variant layout 与 `as` cast failure contract 作为 effect/runtime verifier-backed invariant；NoGC/unsafe explicit-frame spill mirrors、slot count、value/frame count 与 raw value-primitive boundary payload 改为内部 invariant panic，不再复用 UMB diagnostic。
  - Inventory/ledger：active 652 -> 633；retired 632 -> 651；B-33/B-34/B-35 active count 均为 0；`InternalBugSentinel` active 449 -> 430。
  - Stale count：tracked stale total 285 -> 266；`mir_body/cast.rs` 7 -> 6；`mir_body/member.rs` 29 -> 25；`mir_body/terminator.rs` 3 -> 2；`effect_lowered/body/payload.rs` 2 -> 1；`effect_lowered/body/runtime_error.rs` 3 -> 0；`main/declare.rs` 2 -> 0；`main/frame.rs` 3 -> 1；`main/globals.rs` 2 -> 0；`main/immut_value.rs` 2 -> 0；`main/runtime_error.rs` 3 -> 2。
  - Fixture 状态：B-33/B-34/B-35 targeted fixtures active 并通过；retired IDs 改由 retired ledger 覆盖，active fixture `COVERS` 不再引用 retired IDs；B-33 calling-convention/FunPtr fixture 与 B-35 cross-bucket low-level fixtures 保持 `IGNORE-UNTIL-FIX` skip，等待 B-30/相关低层 intrinsic 任务。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-33/B-34/B-35` 均通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，633 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=633、retired=651、initial=1,284）；`cargo test -p scoopc mir::materialize -- --nocapture` 通过（51 passed）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-33-extern-global-funptr/` 通过（4 passed，其中 1 个 skip/pass）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-34-runtime-error-try/` 通过（3 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-35-unsafe-nogc-boundary/` 通过（10 passed，其中 6 个 skip/pass）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:135-159` 与本任务完成条件；B-33/B-34/B-35 active count 为 0，extern、RuntimeError、NoGC/frame boundary contract 不再由 LLVM `UnsupportedMainBody` 兜底。

### [DONE] P7-B2.8：B-08 internal/B-11 member store 与 pure/plain statement route contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-08.md`、`audit/strategies/B-11.md`。
- 范围：B-08 internal rows 4 entries；B-11 14 entries；合计 18 entries。
- 目标：member store internal rows、pure/plain statement route contract 在 MIR/HIR verifier 后不可达。
- 必须实现：member store receiver/place/value invariant、pure/plain statement boundary verifier、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-08-member-store/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-11-pure-boundary/`。
- 完成条件：B-08 internal rows 与 B-11 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/mir/materialize/validation.rs`、`crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/pipeline/hir_completeness.rs`、`crates/scoopc/src/pipeline/hir_stage.rs`、`crates/scoopc/src/llvm/codegen/mir_body/member.rs`、`crates/scoopc/src/llvm/codegen/stmt.rs`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-08.md,B-11.md,_overview.md}`、`audit/strategies/{B-08.md,B-11.md}`、`audit/spec_coverage_matrix.md`、B-08/B-11/B-10/B-20 fixtures 与 `tests/fixtures/umb_fix/_index.csv`；同步记录 `memory/claude_plan.md`。
  - Retired IDs：B-08 `UMB-1132`、`UMB-1133`、`UMB-1146`、`UMB-1148`；B-11 `UMB-1250`、`UMB-1251`、`UMB-1252`、`UMB-1253`、`UMB-1254`、`UMB-1255`、`UMB-1256`、`UMB-1257`、`UMB-1258`、`UMB-1259`、`UMB-1260`、`UMB-1261`、`UMB-1262`、`UMB-1268`；数量 18；bucket `B-08`/`B-11`；class `InternalBugSentinel`。
  - 核心决策：B-08 member-store receiver/place/value contract 进入 production MIR 与 materialized MIR verifier；B-11 local val、assignment place、mutable target、while Bool condition route contract 进入 typed HIR completeness verifier；LLVM lowering 删除对应 `UnsupportedMainBody` fallback，改为 verifier 后的 internal invariant panic/`unreachable!`。
  - Inventory/ledger：active 633 -> 615；retired 651 -> 669；B-08 active 4 -> 0；B-11 active 14 -> 0；`InternalBugSentinel` active 430 -> 412。
  - Stale count：tracked stale total 266 -> 262；`crates/scoopc/src/llvm/codegen/mir_body/member.rs` `UnsupportedMainBody` 25 -> 21；`stmt.rs` 14 active inventory rows -> 0（不在 tracked stale file list）。
  - Fixture 状态：B-08 fixture coverage switched to `COVERS: NONE` after retired ledger takeover；B-11 fixtures activated and switched to retired-ledger coverage; cross-bucket B-10 fixture coverage removed retired B-11 IDs while keeping active B-10 IDs.
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-08` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- list --bucket B-11` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，615 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=615、retired=669、initial=1284）；`cargo test -p scoopc mir::materialize -- --nocapture` 通过（54 passed）；`cargo test -p scoopc pipeline::hir_stage -- --nocapture` 通过（30 passed）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-08-member-store/` 通过（4 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-11-pure-boundary/` 通过（3 passed）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:135-159` 与本任务完成条件；B-08 internal rows 与 B-11 active count 均为 0，member-store/pure/plain statement route contract 不再由 LLVM `UnsupportedMainBody` 兜底。

### [DONE] P7-B3.1：B-32/B-31 print/panic/sysroot 与 scalar methods contract

- 参考：`PLAN.md:161-181`、`audit/strategies/B-32.md`、`audit/strategies/B-31.md`。
- 范围：B-32 10 entries；B-31 12 entries；合计 22 entries。
- 目标：sysroot print/panic 桥接与 Float/Int/Char/Bool/String 扩展方法签名稳定。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-32-print-panic-sysroot/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-31-scalar-methods/`。
- 完成条件：B-32/B-31 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-31.md,B-32.md,_overview.md}`、`audit/strategies/{B-31.md,B-32.md}`、`audit/spec_coverage_matrix.md`、B-31/B-32 fixtures、B-17/B-30 cross-coverage fixtures、`tests/fixtures/umb_fix/_index.csv`、`TODO.md` 与 `memory/claude_plan.md`。
  - Retired IDs：B-32 `UMB-0543`、`UMB-0544`、`UMB-0545`、`UMB-0546`、`UMB-0547`、`UMB-0548`、`UMB-0549`、`UMB-0550`、`UMB-0551`、`UMB-0552`；B-31 `UMB-0575`、`UMB-0576`、`UMB-0577`、`UMB-0578`、`UMB-0579`、`UMB-0580`、`UMB-0584`、`UMB-0585`、`UMB-0586`、`UMB-0587`、`UMB-0591`、`UMB-0592`；数量 22；bucket `B-32`/`B-31`；class `InternalBugSentinel`。
  - 核心决策：sysroot print/panic bridge 与 scalar method receiver/arity/value/return contract 由 typecheck/sysroot bridge 和既有 scalar method gate 保证；LLVM builtin lowering 删除对应 `UnsupportedMainBody` fallback，改为 `panic_verified_builtin_contract` 内部 invariant。
  - Inventory/ledger：active 615 -> 593；retired 669 -> 691；B-31 active 12 -> 0；B-32 active 10 -> 0；`InternalBugSentinel` active 412 -> 390。
  - Stale count：`crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs` active inventory rows 58 -> 36；该文件不在 `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` tracked stale list，tracked stale total 保持 262 -> 262。
  - Fixture 状态：B-31/B-32 fixture directories 激活并通过；retired IDs 改由 retired ledger 覆盖，active fixture `COVERS` 不再引用 B-31/B-32 retired IDs；B-17/B-30 cross-coverage fixture 清理 retired B-31/B-32 IDs。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-31` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- list --bucket B-32` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=593、retired=691、initial=1284）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，593 entries）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-31-scalar-methods/` 通过（3 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-32-print-panic-sysroot/` 通过（3 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/` 通过（149 passed）；`cargo run -p scoop -- test tests/fixtures/run-pass/scalar_method_intrinsic_basic.scoop` 通过（1 passed）；`cargo run -p scoop -- test tests/fixtures/run-pass/string_byte_accessors.scoop` 通过（1 passed）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:161-181` 与本任务完成条件；B-32/B-31 active count 均为 0，print/panic/sysroot bridge 与 scalar method contract 不再由 LLVM `UnsupportedMainBody` 兜底。

### [DONE] P7-B3.2：B-28/B-27 thread/sync intrinsic contract

- 参考：`PLAN.md:161-181`、`audit/strategies/B-28.md`、`audit/strategies/B-27.md`。
- 范围：B-28 20 entries；B-27 58 entries；合计 78 entries。
- 目标：thread/sync intrinsic receiver、arity、return type、destroy/create contract 闭合。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-28-thread-intrinsics/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-27-sync-intrinsics/`。
- 完成条件：B-28/B-27 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/codegen/intrinsics/{mod.rs,thread.rs,sync.rs}`、`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-27.md,B-28.md,_overview.md}`、`audit/strategies/{B-27.md,B-28.md}`、`audit/spec_coverage_matrix.md`、B-27/B-28 fixtures、`tests/fixtures/umb_fix/_index.csv` 与 `memory/claude_plan.md`。
  - Retired IDs：B-28 `UMB-0243`、`UMB-0244`、`UMB-0245`、`UMB-0725`、`UMB-0726`、`UMB-0727`、`UMB-0728`、`UMB-0729`、`UMB-0730`、`UMB-0731`、`UMB-0732`、`UMB-0733`、`UMB-0734`、`UMB-0735`、`UMB-0736`、`UMB-0737`、`UMB-0738`、`UMB-0740`、`UMB-0741`、`UMB-0742`；B-27 `UMB-0246`、`UMB-0247`、`UMB-0248`、`UMB-0249`、`UMB-0250`、`UMB-0656`、`UMB-0657`、`UMB-0658`、`UMB-0659`、`UMB-0660`、`UMB-0661`、`UMB-0662`、`UMB-0663`、`UMB-0664`、`UMB-0665`、`UMB-0666`、`UMB-0667`、`UMB-0668`、`UMB-0669`、`UMB-0670`、`UMB-0671`、`UMB-0672`、`UMB-0673`、`UMB-0674`、`UMB-0675`、`UMB-0676`、`UMB-0677`、`UMB-0678`、`UMB-0679`、`UMB-0680`、`UMB-0681`、`UMB-0682`、`UMB-0683`、`UMB-0684`、`UMB-0685`、`UMB-0686`、`UMB-0687`、`UMB-0688`、`UMB-0689`、`UMB-0690`、`UMB-0691`、`UMB-0692`、`UMB-0693`、`UMB-0694`、`UMB-0695`、`UMB-0696`、`UMB-0697`、`UMB-0698`、`UMB-0699`、`UMB-0700`、`UMB-0701`、`UMB-0702`、`UMB-0703`、`UMB-0704`、`UMB-0705`、`UMB-0706`、`UMB-0707`、`UMB-0708`；数量 78；bucket `B-28`/`B-27`；class `InternalBugSentinel`。
  - 核心决策：新增 verified intrinsic contract helper，HIR thread/sync intrinsic lowering 使用 arity/positional-arg invariant 与 `expect_*` value helper；effect-lowered `sleepMillis`、`currentId`、`Once.isDone`、`destroy` 的 B-27/B-28 专属 fallback 改为内部 invariant；`currentId` effect-lowered path 同步按 host `Int` 返回；B-10 generic sync helper rows 保持 active 不顺手退场。
  - Inventory/ledger：active 593 -> 515；retired 691 -> 769；B-28 active 20 -> 0；B-27 active 58 -> 0；`InternalBugSentinel` active 390 -> 312。
  - Stale count：`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs` `UnsupportedMainBody` 131 -> 123；tracked stale total 262 -> 254；`intrinsics/sync.rs` 53 rows与 `intrinsics/thread.rs` 17 rows退场但不在 tracked stale file list。
  - Fixture 状态：B-27/B-28 fixture directories 从 `IGNORE-UNTIL-FIX` 激活；positive fixtures active 并通过；negative fixtures 保留稳定 frontend/typecheck gate；retired IDs 改由 retired ledger 覆盖，active fixture `COVERS` 改为 `NONE`。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-27` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- list --bucket B-28` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，515 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=515、retired=769、initial=1284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-27-sync-intrinsics/` 通过（2 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-28-thread-intrinsics/` 通过（2 passed）；`cargo check -p scoopc` 通过；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:161-181` 与本任务完成条件；B-28/B-27 active count 均为 0，thread/sync intrinsic receiver、arity、return type、destroy/create contract 不再由 LLVM `UnsupportedMainBody` 兜底。

### [DONE] P7-B3.3：B-26 atomic intrinsic contract

- 参考：`PLAN.md:161-181`、`audit/strategies/B-26.md`。
- 范围：B-26，102 entries。
- 目标：atomic intrinsic target mutability、width、ordering、return contract 闭合。
- 必须实现：atomicInt/atomicRef family 的集中签名和 receiver contract，用户错误走 frontend/typecheck，sysroot shape 走 internal sentinel。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-26-atomic-intrinsics/`。
- 完成条件：B-26 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/codegen/intrinsics/atomic.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`、`crates/scoopc/src/llvm/codegen/main/call.rs`、`crates/scoopc/src/typecheck/expr/call/{dispatch.rs,gates.rs}`、`crates/scoopc/src/typecheck/expr/{error.rs,mod.rs,stmt.rs,entry.rs,collect.rs}`、`crates/scoopc/src/typecheck/lower.rs`、`crates/scoopc/src/pipeline/hir_completeness.rs`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-26.md,_overview.md}`、`audit/strategies/B-26.md`、`audit/spec_coverage_matrix.md`、B-26 fixtures、`tests/fixtures/umb_fix/_index.csv` 与 `memory/claude_plan.md`。
  - Retired IDs：`UMB-0255`..`UMB-0273`、`UMB-0287`..`UMB-0295`、`UMB-0298`..`UMB-0314`、`UMB-0492`..`UMB-0540`、`UMB-0775`..`UMB-0780`、`UMB-0784`..`UMB-0785`；数量 102；bucket `B-26`；class `InternalBugSentinel`。
  - 核心决策：raw `__atomicInt*`/`__atomicRef*` 第一实参 addressability 与写目标 mutability 进入 typecheck gate；HIR/effect-lowered atomic lowering 的 arity、target/value/return/ordering drift 改为 verified intrinsic/internal invariant；同步修复泛型 HIR assignment side table 中模板 local id 与实例化 local id 漂移的 verifier 匹配，避免 `Atomic<T>.exchange` 误报。
  - Inventory/ledger：active 515 -> 413；retired 769 -> 871；B-26 active 102 -> 0；`InternalBugSentinel` active 312 -> 210。
  - Stale count：`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs` `UnsupportedMainBody` 123 -> 78；`crates/scoopc/src/llvm/codegen/main/call.rs` 8 -> 0；tracked stale total 254 -> 201；`intrinsics/atomic.rs` 49 rows退场但不在 tracked stale file list。
  - Fixture 状态：B-26 fixture directory 从 `IGNORE-UNTIL-FIX:B-26` 激活；新增 raw atomic target lvalue/mutability negative fixtures；retired IDs 改由 retired ledger 覆盖，active fixture `COVERS` 改为 `NONE`。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-26` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，413 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=413、retired=871、initial=1284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-26-atomic-intrinsics/` 通过（4 passed）；`cargo run -p scoop -- test tests/fixtures/run-pass/unsafe_atomic_int_basic.scoop`、`unsafe_atomic_int_field_lvalue_basic.scoop`、`sysroot_atomic_basic.scoop` 均通过；`cargo run -p scoop -- test tests/fixtures/typecheck/sysroot_atomic_value_rejects_ref_type.scoop` 通过；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:161-181` 与本任务完成条件；B-26 active count 为 0，atomic intrinsic target mutability、width、ordering、return contract 不再由 LLVM `UnsupportedMainBody` 兜底。

### [DONE] P7-B3.4：B-29 GC intrinsic contract

- 参考：`PLAN.md:161-181`、`audit/strategies/B-29.md`。
- 范围：B-29，93 entries。
- 目标：GC.handleNew/handleGet/handleDrop/pin/unpin 类型和 frame contract 闭合。
- 必须实现：GC intrinsic signature/receiver/return contract、frame reload/pin invariants、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-29-gc-intrinsics/`、`cargo test -p scoop_runtime -- --nocapture`。
- 完成条件：B-29 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/codegen/{gc.rs,effect_outcome.rs,effect_lowered/value.rs,intrinsics/{mod.rs,named.rs},main/{context.rs,frame.rs,gc_locals.rs},mir_body/member.rs}`、`crates/scoopc/src/mir/{mod.rs,materialize/validation.rs}`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-29.md,_overview.md}`、`audit/strategies/B-29.md`、`audit/spec_coverage_matrix.md`、B-29 fixtures、`tests/fixtures/umb_fix/_index.csv` 与 `memory/claude_plan.md`。
  - Retired IDs：`UMB-0340`、`UMB-0341`、`UMB-0342`、`UMB-0343`、`UMB-0346`、`UMB-0347`、`UMB-0348`、`UMB-0349`、`UMB-0350`、`UMB-0351`、`UMB-0371`、`UMB-0426`、`UMB-0427`、`UMB-0428`、`UMB-0429`、`UMB-0430`、`UMB-0431`、`UMB-0432`、`UMB-0435`、`UMB-0436`、`UMB-0437`、`UMB-0438`、`UMB-0439`、`UMB-0440`、`UMB-0441`、`UMB-0442`、`UMB-0443`、`UMB-0446`、`UMB-0447`、`UMB-0448`、`UMB-0449`、`UMB-0450`、`UMB-0451`、`UMB-0452`、`UMB-0453`、`UMB-0454`、`UMB-0455`、`UMB-0458`、`UMB-0459`、`UMB-0460`、`UMB-0461`、`UMB-0462`、`UMB-0463`、`UMB-0464`、`UMB-0465`、`UMB-0466`、`UMB-0467`、`UMB-0470`、`UMB-0471`、`UMB-0472`、`UMB-0473`、`UMB-0474`、`UMB-0475`、`UMB-0476`、`UMB-0477`、`UMB-0478`、`UMB-0481`、`UMB-0482`、`UMB-0483`、`UMB-0484`、`UMB-0485`、`UMB-0486`、`UMB-0487`、`UMB-0488`、`UMB-0489`、`UMB-0490`、`UMB-0491`、`UMB-0644`、`UMB-0654`、`UMB-0882`、`UMB-0900`、`UMB-0901`、`UMB-1149`、`UMB-1150`、`UMB-1151`、`UMB-1152`、`UMB-1153`、`UMB-1154`、`UMB-1157`、`UMB-1158`、`UMB-1159`、`UMB-1160`、`UMB-1161`、`UMB-1162`、`UMB-1163`、`UMB-1164`、`UMB-1167`、`UMB-1168`、`UMB-1169`、`UMB-1170`、`UMB-1171`、`UMB-1172`、`UMB-1173`；数量 93；bucket `B-29`；class `InternalBugSentinel`。
  - 核心决策：复用 typecheck 的 `GC.pin`/`GC.unpin`/`GC.handle*` 用户面签名和 unsafe/NoGC gate；MIR production/materialized verifier 补齐 GC intrinsic argument shape、subject transport、token/result、root lifetime 和 pairing contract；LLVM HIR/MIR/effect-lowered/named intrinsic/frame/write-barrier paths 改为 `expect_*` / verified intrinsic panic boundary。
  - Inventory/ledger：active 413 -> 320；retired 871 -> 964；B-29 active 93 -> 0；`InternalBugSentinel` active 210 -> 117。
  - Stale count：tracked stale total 201 -> 167；`effect_lowered/value.rs` 78 -> 68；`mir_body/member.rs` 21 -> 0；`main/frame.rs` 1 -> 0；`main/gc_locals.rs` 2 -> 0；`gc.rs` 另删除 56 个 active inventory rows、`intrinsics/named.rs` 删除 2 rows、`effect_outcome.rs` 删除 1 row（这些文件不在 tracked stale list）。
  - Fixture 状态：B-29 fixture directory 从 `IGNORE-UNTIL-FIX:B-29` 激活；positive/negative fixtures 均 active；`COVERS` 改为 `NONE`，retired IDs 由 retired ledger 覆盖。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-29` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，320 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=320、retired=964、initial=1284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo test -p scoopc mir::materialize -- --nocapture` 通过（54 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-29-gc-intrinsics/` 通过（2 passed）；`cargo test -p scoop_runtime -- --nocapture` 通过；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:161-181` 与本任务完成条件；B-29 active count 为 0，GC.handleNew/handleGet/handleDrop/pin/unpin 类型、frame/root 和 runtime contract 不再由 LLVM `UnsupportedMainBody` 兜底。

### [DONE] P7-B3.5：B-30 named/unsafe/FunPtr/stackmap intrinsic contract

- 参考：`PLAN.md:161-181`、`audit/strategies/B-30.md`。
- 范围：B-30，117 entries。
- 目标：named intrinsic、unsafe/FunPtr、stackmap statepoint contract 闭合。
- 必须实现：FunPtr signature gate、uintPtr/funptr conversion gate、stackmap/statepoint caller/value contract、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-30-named-unsafe-funptr/`、`cargo run -p scoop -- test tests/fixtures/unsafe_nogc/`。
- 完成条件：B-30 active count 为 0。
- 依赖：P7-B1 推荐完成后；B-35 完成后更安全。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/codegen/{call/lowering.rs,gc.rs,effect_lowered/value.rs,intrinsics/{builtin.rs,named.rs,sysroot.rs},mir_body/{aggregates.rs,call.rs,value_args.rs},main/context.rs}`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-29.md,B-30.md,_overview.md}`、`audit/strategies/B-30.md`、`audit/spec_coverage_matrix.md`、B-30/B-35 fixtures、`tests/fixtures/umb_fix/_index.csv`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 与 `memory/claude_plan.md`。
  - Retired IDs：`UMB-0031`、`UMB-0033`、`UMB-0034`、`UMB-0037`、`UMB-0038`、`UMB-0051`、`UMB-0052`、`UMB-0354`..`UMB-0357`、`UMB-0422`..`UMB-0425`、`UMB-0553`..`UMB-0567`、`UMB-0570`、`UMB-0572`..`UMB-0574`、`UMB-0581`..`UMB-0583`、`UMB-0588`..`UMB-0590`、`UMB-0593`..`UMB-0603`、`UMB-0613`..`UMB-0629`、`UMB-0631`..`UMB-0643`、`UMB-0645`..`UMB-0653`、`UMB-0655`、`UMB-0709`..`UMB-0717`、`UMB-0719`..`UMB-0724`、`UMB-0977`..`UMB-0979`、`UMB-0997`、`UMB-1007`..`UMB-1009`、`UMB-1019`、`UMB-1228`..`UMB-1230`；数量 117；bucket `B-30`；class `InternalBugSentinel`。
  - 核心决策：复用 typecheck 的 `FunPtr<F>` signature/unsafe context gate、generic replay gate、named intrinsic metadata contract 与 materialized MIR FunPtr call verifier；LLVM HIR/MIR/effect-lowered lowering 删除 B-30 `UnsupportedMainBody` fallback，改为 `panic_verified_intrinsic_contract`、`expect_*` helper 或明确 internal invariant panic。
  - Inventory/ledger：active 320 -> 203；retired 964 -> 1081；B-30 active 117 -> 0；`InternalBugSentinel` active 117 -> 0；P7-B `InternalBugSentinel` 退场完成，剩余 active 均为 P7-C `RealImpl`。
  - Stale count：tracked stale total 167 -> 152；`effect_lowered/value.rs` 68 -> 64；`mir_body/aggregates.rs` 8 -> 5；`mir_body/call.rs` 13 -> 8；`mir_body/value_args.rs` 6 -> 3；`intrinsics/named.rs`、`intrinsics/builtin.rs`、`intrinsics/sysroot.rs`、`call/lowering.rs`、`gc.rs` 删除目标 B-30 rows 但不在 tracked stale list。
  - Fixture 状态：B-30 fixture directory 从 `IGNORE-UNTIL-FIX:B-30` 激活；B-30 fixtures 与 B-35 raw-pointer cross fixture 的 `COVERS` 改为 `NONE`，retired IDs 由 retired ledger 覆盖。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-30` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=203、retired=1081、initial=1284）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，203 entries）；`cargo check -p scoopc` 通过；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-30-named-unsafe-funptr/` 通过（4 passed）；`cargo run -p scoop -- test tests/fixtures/unsafe_nogc/` 通过（34 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-35-unsafe-nogc-boundary/` 通过（10 passed，其中 4 个 skip/pass）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:161-181` 与本任务完成条件；B-30 active count 为 0，named/unsafe/FunPtr/stackmap intrinsic contract 不再由 LLVM `UnsupportedMainBody` 兜底，P7-B `InternalBugSentinel` active count 清零。

## P7-C：RealImpl 退场（203 entries）

### [DONE] P7-C1：B-24 Reflection / comptime intrinsic 实现

- 参考：`PLAN.md:183-206`、`audit/strategies/B-24.md`。
- 范围：B-24，6 entries，`RealImpl`。
- 目标：实现 `sizeOf`、`kindOf`、`descOf`、comptime metadata 参数/返回 contract。
- 必须实现：合法 positive fixture 走真实 codegen；移除对应 `IGNORE-UNTIL-FIX:B-24`；更新 `_index.csv` status。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-24-reflection-comptime/`。
- 完成条件：B-24 active count 为 0；B-24 positive fixture active 并通过。
- 依赖：P7-0-T02；相关 B 类 verifier 完成后更安全。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs`、`crates/scoopc/src/llvm/codegen/mir_body/aggregates.rs`、`crates/scoop/src/fixtures/mod.rs`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-12.md,B-24.md,_overview.md}`、`audit/strategies/B-24.md`、`audit/spec_coverage_matrix.md`、B-24 fixtures、`tests/fixtures/umb_fix/_index.csv`、`TODO.md` 与 `memory/claude_plan.md`。
  - Retired IDs：`UMB-0568`、`UMB-0569`、`UMB-0571`、`UMB-0954`、`UMB-0956`、`UMB-0957`；数量 6；bucket `B-24`；class `RealImpl`。
  - 核心决策：`sizeOf<T>()` 使用 typed call-site 的 type argument 走真实静态 size lowering，legacy `sizeOf(value)` 继续只消费 value 的静态类型；HIR/MIR `sizeOf`/`kindOf`/`descOf` type/binding drift 改为 verified intrinsic contract panic boundary，不再暴露 UMB diagnostic；`umb_fix` 增加 `UMB-FIX-PHASE: comptime` 以激活 B-24 const-eval 正/负例。
  - Inventory/ledger：active 203 -> 197；retired 1081 -> 1087；B-24 active 6 -> 0；`RealImpl` active 203 -> 197。
  - Stale count：tracked stale total 152 -> 149；`crates/scoopc/src/llvm/codegen/mir_body/aggregates.rs` `UnsupportedMainBody` 5 -> 2；`intrinsics/builtin.rs` 另删除 3 个 B-24 inventory rows（不在 tracked stale list）。
  - Fixture 状态：B-24 fixture directory 从 `IGNORE-UNTIL-FIX:B-24` 激活；新增 `pos_reflection_codegen.scoop` runtime codegen smoke 与 comptime golden files；active fixture `COVERS` 改为 `NONE`，retired IDs 由 retired ledger 覆盖。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-24` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，197 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=197、retired=1087、initial=1284）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-24-reflection-comptime/` 通过（11 passed）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:183-206` 与本任务完成条件；B-24 active count 为 0，B-24 positive fixtures active 并通过，supported reflection/comptime intrinsic happy path 不再由 LLVM `UnsupportedMainBody` 兜底。

### [DONE] P7-C2：B-25 Platform / RTTI intrinsic 实现

- 参考：`PLAN.md:183-206`、`audit/strategies/B-25.md`。
- 范围：B-25，14 entries，`RealImpl`。
- 目标：runtime type descriptor、runtime type check metadata、`as?` target runtime type 支持真实 codegen。
- 必须实现：RTTI metadata materialization、runtime type operand/target lowering、fixture active。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-25-platform-rtti/`。
- 完成条件：B-25 active count 为 0；B-25 positive fixture active 并通过。
- 依赖：P7-C1 推荐完成后；B-14/B-22/B-23 contract 稳定后更安全。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/codegen/mir_body/{cast.rs,const_pat.rs,transport.rs}`、`crates/scoopc/src/mir/materialize/validation.rs`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-25.md,_overview.md}`、`audit/strategies/B-25.md`、`audit/spec_coverage_matrix.md`、B-25 fixtures、`tests/fixtures/umb_fix/_index.csv`、`TODO.md` 与 `memory/claude_plan.md`。
  - Retired IDs：`UMB-1044`、`UMB-1057`、`UMB-1060`、`UMB-1061`、`UMB-1062`、`UMB-1063`、`UMB-1086`、`UMB-1089`、`UMB-1215`、`UMB-1216`、`UMB-1217`、`UMB-1218`、`UMB-1219`、`UMB-1220`；数量 14；bucket `B-25`；class `RealImpl`。
  - 核心决策：复用 materialized MIR runtime type/cast verifier 与 TypeStore equivalence gate；为 `is` pattern 补齐 runtime descriptor shape 与 dynamic codegen support 校验；MIR `is`/`as?`/pattern RTTI 和 `getPlatform()` Platform literal lowering 删除 B-25 `UnsupportedMainBody` fallback，改为 verifier/sysroot contract 后的 internal invariant panic，同时保留真实 runtime descriptor 和 Platform field materialization。
  - Inventory/ledger：active 197 -> 183；retired 1087 -> 1101；B-25 active 14 -> 0；`RealImpl` active 197 -> 183。
  - Stale count：tracked stale total 149 -> 135；`crates/scoopc/src/llvm/codegen/mir_body/cast.rs` `UnsupportedMainBody` 6 -> 0；`const_pat.rs` 2 -> 0；`transport.rs` 8 -> 2。
  - Fixture 状态：B-25 fixture directory 从 `IGNORE-UNTIL-FIX:B-25` 激活；`pos_platform_rtti_runtime.scoop` 扩展为同时覆盖 `getPlatform()`、`is`、`!is`、`as?` 与 `is` pattern；negative fixture 改为 active frontend/typecheck invalid cast gate；active fixture `COVERS` 改为 `NONE`，retired IDs 由 retired ledger 覆盖。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-25` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，183 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=183、retired=1101、initial=1284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo test -p scoopc mir::materialize -- --nocapture` 通过（54 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-25-platform-rtti/` 通过（2 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-14-cast-typecheck/pos_cast_typecheck_runtime.scoop` 通过（1 passed）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:183-206` 与本任务完成条件；B-25 active count 为 0，B-25 positive fixture active 并通过，Platform / RTTI happy paths 不再由 LLVM `UnsupportedMainBody` 兜底。

### [DONE] P7-C3：B-13 数组 / 复合 transport metadata 实现

- 参考：`PLAN.md:183-206`、`audit/strategies/B-13.md`。
- 范围：B-13，24 entries，`RealImpl`。
- 目标：task transport tuple、resume payload、composed call replay block、array metadata 支持真实 codegen。
- 必须实现：transport tuple metadata、resume payload lowering、composed call replay lowering、fixture active。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-13-composite-transport/`。
- 完成条件：B-13 active count 为 0；B-13 positive fixture active 并通过。
- 依赖：B-03/B-09/B-14 contract 稳定后更安全。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/codegen/{effect_outcome.rs,effect_lowered/{body/{composed_call.rs,payload.rs},value.rs},intrinsics/named.rs,main/{context.rs,runtime_error.rs},mir_body/transport.rs}`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-13.md,_overview.md}`、`audit/strategies/B-13.md`、`audit/spec_coverage_matrix.md`、B-13 fixtures、B-06/B-21 cross-coverage fixtures、`tests/fixtures/umb_fix/_index.csv` 与 `memory/claude_plan.md`。
  - Retired IDs：`UMB-0198`、`UMB-0214`、`UMB-0315`、`UMB-0364`、`UMB-0365`、`UMB-0366`、`UMB-0372`、`UMB-0374`、`UMB-0375`、`UMB-0376`、`UMB-0379`、`UMB-0380`、`UMB-0381`、`UMB-0605`、`UMB-0606`、`UMB-0608`、`UMB-0610`、`UMB-0611`、`UMB-0612`、`UMB-0630`、`UMB-0945`、`UMB-0946`、`UMB-1211`、`UMB-1212`；数量 24；bucket `B-13`；class `RealImpl`。
  - 核心决策：为 array/composite transport 使用 descriptor-backed lowering；task transport tuple encode/decode、resume payload、composed-call replay source block、array intrinsic arity/value/stride 和 composite effect payload boxing 改为真实 lowering 或 verified invariant；Float value erasure 走 MIR value box；optional int-literal source-span probe 对合成 span 返回 `None`，避免合法 composite transport 路径触发 source-slice invariant。
  - Inventory/ledger：active 183 -> 159；retired 1101 -> 1125；B-13 active 24 -> 0；`RealImpl` active 183 -> 159。
  - Stale count：tracked stale total 135 -> 128；`effect_lowered/value.rs` 64 -> 63；`effect_lowered/body/composed_call.rs` 2 -> 1；`effect_lowered/body/payload.rs` 1 -> 0；`mir_body/transport.rs` 2 -> 0；`main/runtime_error.rs` 2 -> 0；`effect_outcome.rs` 与 `intrinsics/named.rs` 删除目标 B-13 rows 但不在 tracked stale list。
  - Fixture 状态：B-13 fixture directory active；新增 runtime stdout golden files；B-13 active fixture `COVERS` 改为 `NONE`，retired IDs 由 retired ledger 覆盖；B-06/B-21 cross-coverage 清理 retired B-13 IDs。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-13` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，159 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=159、retired=1125、initial=1284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-13-composite-transport/` 通过（4 passed）；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:183-206` 与本任务完成条件；B-13 active count 为 0，B-13 positive fixtures active 并通过，task transport tuple、resume payload、composed call replay block 和 array metadata 不再由 LLVM `UnsupportedMainBody` 兜底。

### [DONE] P7-C4：B-12 Closure / lambda / capture 实现

- 参考：`PLAN.md:183-206`、`audit/strategies/B-12.md`。
- 范围：B-12，50 entries，`RealImpl`。
- 目标：closure env、mutable capture、non-scalar capture、callable lookup、lambda return 支持真实 codegen。
- 必须实现：closure env layout、capture materialization、non-scalar transport、callable lookup、lambda return lowering、fixture active。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-12-closure-capture/`、`cargo run -p scoop -- test tests/fixtures/run-pass/`。
- 完成条件：B-12 active count 为 0；B-12 positive fixture active 并通过。
- 依赖：B-03/B-09/B-13 contract 稳定后更安全。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/codegen/{closure/mod.rs,main/identity.rs,mir_body/{aggregates.rs,call.rs,callable_lookup.rs,operand.rs,terminator.rs,value_args.rs}}`、`crates/scoopc/src/mir/{lower/mir_lowering_facts.rs,materialize/{generic_mir.rs,instance.rs,run.rs,validation.rs}}`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-10.md,B-12.md,_overview.md}`、`audit/strategies/B-12.md`、`audit/spec_coverage_matrix.md`、B-12 fixtures、`tests/fixtures/umb_fix/_index.csv` 与 `memory/claude_plan.md`。
  - Retired IDs：`UMB-0092`、`UMB-0094`..`UMB-0096`、`UMB-0099`..`UMB-0100`、`UMB-0103`..`UMB-0115`、`UMB-0917`..`UMB-0920`、`UMB-0971`、`UMB-0976`、`UMB-0995`、`UMB-1005`、`UMB-1013`、`UMB-1023`..`UMB-1024`、`UMB-1027`..`UMB-1028`、`UMB-1032`、`UMB-1035`..`UMB-1037`、`UMB-1041`..`UMB-1043`、`UMB-1184`..`UMB-1189`、`UMB-1203`、`UMB-1207`、`UMB-1225`..`UMB-1227`；数量 50；bucket `B-12`；class `RealImpl`。
  - 核心决策：closure object/env 使用 GC-managed object layout；direct HIR lambda 与 materialized MIR closure 统一发布 stable closure identity、env layout、callable lookup 和 hidden-sret/ordinary param ABI；non-scalar capture 通过 env object field 与 tuple/composite transport descriptor 真实 lower；materialized MIR 保留非泛型 metadata 子树用于 composite capture validation，并在 metadata 未发布 receiver ABI param 时退回到结果 transport 校验。
  - Inventory/ledger：active 159 -> 109；retired 1125 -> 1175；B-12 active 50 -> 0；`RealImpl` active 159 -> 109。
  - Stale count：tracked stale total 128 -> 97；`mir_body/aggregates.rs` 2 -> 0；`mir_body/callable_lookup.rs` 11 -> 0；`mir_body/operand.rs` 6 -> 0；`mir_body/terminator.rs` 2 -> 0；`mir_body/value_args.rs` 3 -> 0；`mir_body/call.rs` active B-12 rows retired but B-10 rows remain active at 5；`closure/mod.rs` 与 `main/identity.rs` 删除目标 B-12 inventory rows 但不在 tracked stale list。
  - Fixture 状态：B-12 fixture directory active；`pos_lambda_closure_capture.scoop` 使用 stdout golden；`neg_lambda_capture_contract.scoop` 保持 active typecheck negative coverage；B-12 active fixture `COVERS` 改为 `NONE`，retired IDs 由 retired ledger 覆盖。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-12` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，109 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=109、retired=1175、initial=1284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo test -p scoopc mir::materialize -- --nocapture` 通过（54 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-12-closure-capture/` 通过（2 passed）；`cargo run -p scoop -- test tests/fixtures/run-pass/closure_env_composite_capture_basic.scoop` 通过（1 passed）；`cargo run -p scoop -- test tests/fixtures/run-pass/` 仍失败（307/416 passed，109 failed），抽样 `effect_escape_continuation_resume_string.scoop`、`effect_escape_continuation_indirect_perform_closure_locals.scoop`、`effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop` 在 clean `HEAD` worktree 同样失败，判定为非 P7-C4 引入的历史/future-task failures；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:183-206` 与本任务完成条件；B-12 active count 为 0，B-12 positive fixture active 并通过，closure env、mutable/non-scalar capture、callable lookup 与 lambda return happy paths 不再由 LLVM `UnsupportedMainBody` 兜底。

### [DONE] P7-C5：B-10 Effect-typed callable adapter / ABI routing 实现

- 参考：`PLAN.md:183-206`、`audit/strategies/B-10.md`。
- 范围：B-10，109 entries，`RealImpl`。
- 目标：effect callable adapter、continuation carrier、resume token、effect outcome slot、surface function ABI 支持真实 codegen。
- 必须实现：effect callable adapter、continuation carrier lowering、resume token plumbing、effect outcome ABI slot、surface function adapter、fixture active。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-10-effect-callable-adapter/`、`cargo run -p scoop -- test tests/fixtures/effect_lowered/`、`cargo run -p scoop -- test`。
- 完成条件：B-10 active count 为 0；B-10 positive fixture active 并通过；`tests/fixtures/umb_fix/**` 不再有 `IGNORE-UNTIL-FIX`。
- 依赖：P7-C1、P7-C2、P7-C3、P7-C4；B-03/B-09/B-13 contract 稳定后再推进。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/codegen/{call/{abi.rs,lowering.rs},expr.rs,effect_lowered/{body/{call_invoke.rs,composed_call.rs,main_carrier.rs,main_entry.rs,states.rs,wrapper.rs},value.rs},mir_body/{call.rs,dispatch.rs,member.rs,mod.rs,types.rs},ordinary_callee.rs}`、`crates/scoopc/src/mir/{mod.rs,materialize/{validation.rs,tests.rs}}`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`；同步 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、`audit/UMB_categories/{B-10.md,_overview.md}`、`audit/strategies/B-10.md`、`audit/spec_coverage_matrix.md`、B-10/B-21/B-33/B-35 fixtures、`tests/fixtures/umb_fix/_index.csv` 与 `memory/claude_plan.md`。
  - Retired IDs：`UMB-0006`、`UMB-0007`、`UMB-0022`、`UMB-0029`、`UMB-0194`、`UMB-0196`、`UMB-0197`、`UMB-0199`..`UMB-0213`、`UMB-0219`..`UMB-0224`、`UMB-0226`..`UMB-0238`、`UMB-0240`..`UMB-0242`、`UMB-0251`..`UMB-0254`、`UMB-0274`..`UMB-0286`、`UMB-0316`..`UMB-0339`、`UMB-0358`..`UMB-0363`、`UMB-0419`..`UMB-0421`、`UMB-1014`、`UMB-1015`、`UMB-1017`、`UMB-1018`、`UMB-1020`、`UMB-1111`、`UMB-1116`、`UMB-1179`..`UMB-1181`、`UMB-1245`..`UMB-1249`；数量 109；bucket `B-10`；class `RealImpl`。
  - 核心决策：B-10 production `UnsupportedMainBody` constructor 全部移除；effect callable adapter、plain/effectful closure adapter、continuation carrier、resume token、effect outcome ABI slot、surface/plain callable routing 走真实 lowering 或 verifier-backed internal invariant；补齐跨 TypeStore/default `eff Pure` 等价判断，避免合法 effect/closure/continuation 路径被 materialized verifier 误拒。
  - Inventory/ledger：active 109 -> 0；retired 1175 -> 1284；B-10 active 109 -> 0；`RealImpl` active 109 -> 0；P7-A/P7-B/P7-C active inventory 全部清零。
  - Stale count：tracked stale total 97 -> 0；`mir_body/call.rs` 5 -> 0；`mir_body/dispatch.rs` 2 -> 0；`mir_body/mod.rs` 3 -> 0；`effect_lowered/body/call_invoke.rs` 3 -> 0；`effect_lowered/body/composed_call.rs` 1 -> 0；`effect_lowered/body/main_carrier.rs` 5 -> 0；`effect_lowered/body/main_entry.rs` 9 -> 0；`effect_lowered/body/states.rs` 5 -> 0；`effect_lowered/body/wrapper.rs` 1 -> 0；`effect_lowered/value.rs` 63 -> 0。
  - Fixture 状态：B-10 fixture directory 从 `IGNORE-UNTIL-FIX:B-10` 激活；B-21/B-33/B-35 residual `umb_fix` ignored fixtures 同步激活；`tests/fixtures/umb_fix/**` 不再包含 `IGNORE-UNTIL-FIX`；B-10 active fixture `COVERS` 改为 `NONE`，retired IDs 由 retired ledger 覆盖。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- list --bucket B-10` 通过（entries 0）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=0、retired=1284、initial=1284）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，0 entries）；`cargo test -p scoopc audit:: -- --nocapture` 通过（23 passed）；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-10-effect-callable-adapter/` 通过（7 passed）；`cargo run -p scoop -- test tests/fixtures/effect_lowered/` 通过（10 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/` 通过（152 passed，无 skip）；`cargo run -p scoop -- test tests/fixtures/run-pass/ --exit-on-failure` 通过（416 passed）；`cargo run -p scoop -- test` 通过（fixtures ok，1558 checks）；`cargo test --all --all-targets` 通过；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:183-206` 与本任务完成条件；B-10 active count 为 0，B-10 positive fixtures active 并通过，`tests/fixtures/umb_fix/**` 无 `IGNORE-UNTIL-FIX`，P7-C `RealImpl` active count 清零。

## P8：最终退场

### [DONE] P8-T01：删除 `LlvmEmitError::UnsupportedMainBody` enum variant

- 参考：`PLAN.md:208-225`。
- 触发条件：`umb-audit stats` 显示 active=0、retired=1,284。
- 目标：从 production compiler 中物理删除 UMB variant 和相关 diagnostic 映射。
- 必须实现：
  1. 删除 `crates/scoopc/src/llvm/mod.rs` 中 `LlvmEmitError::UnsupportedMainBody` variant。
  2. 删除所有相关 diagnostic code/message 映射。
  3. 确认 `rg -n "UnsupportedMainBody" crates/scoopc/src/llvm` 不再命中 production codegen 路径。
  4. 清理 `pipeline_user_visible_failure_policy` 中 stale unsupported 历史计数，或归档该测试。
- 验证：
  1. `cargo run -p scoopc --bin umb-audit -- stats`
  2. `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  3. `cargo test --all --all-targets`
- 完成条件：UMB enum variant 已删除；stale unsupported total 为 0 或历史测试已归档。
- 依赖：P7-A、P7-B、P7-C 全部完成。
- 完成记录：
  - 改动范围：更新 `crates/scoopc/src/llvm/mod.rs`、`crates/scoopc/src/llvm/codegen_gap_inventory.rs`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs`、`TODO.md` 与 `memory/claude_plan.md`。
  - Retired IDs：无新增；数量 0；P8-T01 不 retire production row，沿用 P7-C5 后 active=0、retired=1284 的 ledger 状态。
  - 核心决策：物理删除 `LlvmEmitError::UnsupportedMainBody` enum variant 与 `scoop::llvm::unsupported_main_body` diagnostic mapping；清理 LLVM gap inventory 中旧 `UnsupportedMainBody` trigger；删除 `pipeline_user_visible_failure_policy` 中已归零的 per-file stale UMB count table 和历史计数测试，保留长期 failure-policy guard。
  - Inventory/ledger：active 0 -> 0；retired 1284 -> 1284；initial 1284。
  - Stale count：`STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 历史计数表与对应测试已删除；`rg -n "UnsupportedMainBody" crates/scoopc/src/llvm` 无命中。
  - Fixture 状态：未改 fixture active/ignore 状态；P8-T02 继续负责最终归档和 `umb_fix` fixture 总确认。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- stats` 通过（active=0、retired=1284、initial=1284）；`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，0 entries）；`rg -n "UnsupportedMainBody" crates/scoopc/src/llvm` 无命中；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过（6 passed）；`cargo test --all --all-targets` 通过；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
  - 闭合目标：满足 `PLAN.md:208-225` 中 P8-T01 要求；UMB enum variant 已删除，相关 diagnostic mapping 已删除，stale unsupported 历史计数测试已清理。

### [DONE] P8-T02：归档 audit ledger 并更新 DONE 记录

- 参考：`PLAN.md:208-245`、`UnsupportedMainBody_DONE.md`。
- 目标：完成 P8 归档和最终退场记录。
- 必须实现：
  1. 将 `audit/UMB_inventory_initial.csv`、`audit/UMB_retired.csv`、最终 empty inventory 和本计划归档到 `docs/archive/`。
  2. 删除或改造只服务于 UMB 退场的 audit tests/bin；保留长期有价值的 fixture coverage 测试。
  3. 确认所有 `umb_fix` fixture active 且无 ignored/xfail 状态。
  4. 更新 `UnsupportedMainBody_DONE.md`，记录 P8 完成时间、最终验证命令和归档位置。
  5. 更新本 TODO 顶部“当前状态”为完成，并补全 P8-T02 完成记录。
- 验证：
  1. `cargo run -p scoop -- test tests/fixtures/umb_fix/`
  2. `cargo test --all --all-targets`
  3. `cargo run -p scoop -- test`
  4. `cargo clippy --all-targets -- -D warnings`
- 完成条件：`PLAN.md:237-245` 的全部完成判据闭合；`UnsupportedMainBody_DONE.md` 有最终记录。
- 依赖：P8-T01。
- 完成记录：
  - 改动范围：删除 `crates/scoopc/src/bin/umb-audit.rs` 与 `crates/scoopc/src/audit/umb_inventory.rs`/`sentinel_tests.rs`，从 `crates/scoopc/Cargo.toml` 移除 `umb-audit` binary；将 `crates/scoopc/src/audit/spec_coverage.rs` 改造为长期 `umb_fix` fixture coverage audit；同步 B-01 sentinel README 与各 `umb_fix` README 的 active 状态说明；新增 `docs/archive/audits/unsupported-main-body/` audit ledger 归档；更新 `UnsupportedMainBody_DONE.md`、`TODO.md` 与 `memory/claude_plan.md`。
  - Retired IDs：无新增；数量 0；P8-T02 不 retire production row，沿用 P7-C5 后 active=0、retired=1284 的 ledger 状态。
  - 核心决策：P8 后 UMB inventory generator/CLI 已无长期职责，删除只服务退场倒计时的 scan/diff/list/stats tooling；保留 fixture index、bucket positive/negative coverage、no ignored/xfail、negative diagnostic forbidden terms、spec matrix path consistency 等长期有价值 audit。
  - Inventory/ledger：active 0 -> 0；retired 1284 -> 1284；initial 1284；`UMB_inventory_initial.csv`、`UMB_retired.csv`、final empty inventory 与 schema 归档到 `docs/archive/audits/unsupported-main-body/`。
  - Stale count：`crates/scoopc/src/llvm/**` 无 `UnsupportedMainBody` 命中；`tests/fixtures/umb_fix/**` 无 `IGNORE-UNTIL-FIX`、`ignore-until-fix`、`XFAIL`、`xfail` 命中；`umb_fix` fixture `COVERS` 均为 `NONE`。
  - Fixture 状态：`tests/fixtures/umb_fix/**` 全部 active；B-01 sentinel-only coverage 指向归档 retired ledger；`cargo run -p scoop -- test tests/fixtures/umb_fix/` 通过（152 fixtures）。
  - 验证结果：`cargo run -p scoopc --bin umb-audit -- stats` 在删除工具前通过（active=0、retired=1284、initial=1284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（12 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/` 通过（152 passed）；`cargo test --all --all-targets` 通过；`cargo run -p scoop -- test` 通过（fixtures ok，1558 checks）；`cargo clippy --all-targets -- -D warnings` 通过；`cargo fmt` 通过。
  - 闭合目标：满足 `PLAN.md:208-225` 与 `PLAN.md:237-245`；P8 audit ledger 和计划归档完成，`UnsupportedMainBody_DONE.md` 已记录最终验证和归档位置，P7/P8 全部完成。

## 阶段性统计目标

| 阶段 | Active 目标 | Retired 目标 | 备注 |
|---|---:|---:|---|
| 当前 | 0 | 1284 | P8-T02 完成，`LlvmEmitError::UnsupportedMainBody` enum variant 已删除，ledger/plan 已归档 |
| P7-A 完成 | 1,159 | 125 | `FrontendReject` 清零 |
| P7-B 完成 | 203 | 1,081 | `InternalBugSentinel` 清零 |
| P7-C 完成 | 0 | 1,284 | `RealImpl` 清零 |
| P8 完成 | 0 | 1,284 | enum variant 删除，ledger 归档 |

## 最终完成判据

- Active `audit/UMB_inventory.csv` 行数为 0。
- Retired ledger 覆盖全部 1,284 个 initial `UMB-NNNN`，无重复、无缺失。
- `FrontendReject`、`InternalBugSentinel`、`RealImpl` 三类 active count 全为 0。
- `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 总数为 0 或已随 P8 归档删除。
- 所有 `tests/fixtures/umb_fix/**` fixture active 且通过。
- `LlvmEmitError::UnsupportedMainBody` enum variant 已删除。
- `cargo test --all --all-targets`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings` 全部通过。
- `UnsupportedMainBody_DONE.md` 记录最终验证结果与归档位置。
