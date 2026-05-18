# TODO（UnsupportedMainBody Production 修复计划，P7/P8）

> 生成时间：2026-05-18
> 计划基线：[`PLAN.md`](./PLAN.md)
> 上一阶段任务档案：[`TODO-1.md`](./TODO-1.md)
> 设计与 baseline：[`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md)、[`UnsupportedMainBody_DONE.md`](./UnsupportedMainBody_DONE.md)
> 当前状态：P7-0-T01、P7-0-T02 已完成；production 修复尚未开始。

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
- `audit/UMB_inventory.csv`：当前 active inventory，1,284 条。
- `audit/UMB_categories/_overview.md`：bucket entry 数和 class 分布总览。
- `audit/UMB_categories/B-XX.md`：每 bucket symptom、root cause、fixture pointer。
- `audit/strategies/B-XX.md`：每 bucket 上游契约和 P7 修复策略。
- `tests/fixtures/umb_fix/_index.csv`：fixture 到 UMB id 的覆盖索引。
- `UnsupportedMainBody_DONE.md`：P7/P8 handoff 和最终退场记录位置。

### 工具与测试入口

- `cargo run -p scoopc --bin umb-audit -- stats`：统计 active inventory。
- `cargo run -p scoopc --bin umb-audit -- diff`：源码与 inventory 对账。
- `cargo run -p scoopc --bin umb-audit -- list --bucket B-XX`：列出 bucket 待退场 ID。
- `cargo test -p scoopc audit:: -- --nocapture`：P1-P6 audit baseline。
- `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`：用户可见失败策略和 stale count baseline。
- `cargo run -p scoop -- test tests/fixtures/umb_fix/`：UMB fixture 集合。
- `cargo test --all --all-targets`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`：阶段性全量验证。

### Production 关键位置

- `crates/scoopc/src/llvm/mod.rs`：`LlvmEmitError::UnsupportedMainBody` enum variant 和 diagnostic 映射。
- `crates/scoopc/src/llvm/codegen/**`：当前 1,284 个 `UnsupportedMainBody` constructor 所在路径。
- `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`：`STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 与 forbidden terms baseline。
- `crates/scoopc/src/audit/umb_inventory.rs`：当前 inventory scanner、baseline constants 和 audit tests。
- `crates/scoopc/src/bin/umb-audit.rs`：inventory CLI。

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

### [TODO] P7-A1：B-16 控制流 outside-of-context 早拒

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
- 完成记录：待填写。

### [TODO] P7-A2：B-08/B-21 成员写入与 struct 字段负例早拒

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
- 完成记录：待填写。

### [TODO] P7-A3：B-15 when / 模式匹配用户面早拒

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
- 完成记录：待填写。

### [TODO] P7-A4：B-36 spec-uncovered surface 早拒

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
- 完成记录：待填写。

## P7-B：InternalBugSentinel 退场（956 entries）

### [TODO] P7-B1：B-01 helper invariant 统一迁移

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
- 完成记录：待填写。

### [TODO] P7-B2.1：B-02/B-04 MIR local、param、return type contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-02.md`、`audit/strategies/B-04.md`。
- 范围：B-02 6 entries；B-04 29 entries；合计 35 entries。
- 目标：local、param、return type 在 MIR materialize/strict verifier 后完整，不由 codegen 兜底。
- 必须实现：MIR 类型完整性 verifier、param/return arity 和 type gate、codegen fallback retire、inventory/ledger/stale count 对账。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-02-mir-local-type/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-04-function-signature/`。
- 完成条件：B-02/B-04 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：待填写。

### [TODO] P7-B2.2：B-05 MIR CFG contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-05.md`。
- 范围：B-05，25 entries。
- 目标：CFG start block、goto/branch target、terminator shape 在 MIR verifier 后合法。
- 必须实现：CFG verifier gate、target existence/arity check、terminator type check、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-05-mir-cfg/`。
- 完成条件：B-05 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：待填写。

### [TODO] P7-B2.3：B-06/B-07/B-21 aggregate、pattern、field schema contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-06.md`、`audit/strategies/B-07.md`、`audit/strategies/B-21.md`。
- 范围：B-06 43 entries；B-07 34 entries；B-21 internal rows 3 entries；合计 80 entries。
- 目标：struct/tuple/enum literal、pattern clause、field schema 在 MIR verifier 后闭合。
- 必须实现：aggregate schema verifier、pattern arity/type verifier、field index/name verifier、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-06-literal-schema/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-07-pattern-schema/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-21-struct-fields/`。
- 完成条件：B-06/B-07/B-21 internal rows active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：待填写。

### [TODO] P7-B2.4：B-03/B-09/B-14 call ABI、TypeStore、cast contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-03.md`、`audit/strategies/B-09.md`、`audit/strategies/B-14.md`。
- 范围：B-03 56 entries；B-09 13 entries；B-14 27 entries；合计 96 entries。
- 目标：direct/closure/funptr call ABI、cross-TypeStore equivalence、`as`/`as?`/`is` contract 闭合。
- 必须实现：集中 TypeStore equivalence helper、call ABI verifier、cast/typecheck verifier、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-03-call-abi/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-09-type-equivalence/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-14-cast-typecheck/`。
- 完成条件：B-03/B-09/B-14 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：待填写。

### [TODO] P7-B2.5：B-17/B-18 scalar coercion 与 literal/string contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-17.md`、`audit/strategies/B-18.md`。
- 范围：B-17 47 entries；B-18 4 entries；合计 51 entries。
- 目标：coercion、equality、bool/string operator、literal slice/value contract 不再由 codegen 兜底。
- 必须实现：标量 coercion helper、literal source/value contract、string equality/load contract、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-17-coercion-scalar/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-18-literals-strings/`。
- 完成条件：B-17/B-18 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：待填写。

### [TODO] P7-B2.6：B-19/B-20/B-22/B-23 layout 与 member contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-19.md`、`audit/strategies/B-20.md`、`audit/strategies/B-22.md`、`audit/strategies/B-23.md`。
- 范围：B-19 39 entries；B-20 46 entries；B-22 40 entries；B-23 24 entries；合计 149 entries。
- 目标：top-level/object/class/enum/member layout 和 access contract 在 codegen 前闭合。
- 必须实现：top-level metadata gate、object/class ctor/member layout verifier、enum/niche layout verifier、member receiver/target verifier、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-19-top-level-object-extern/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-20-class-property-field/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-22-enum-niche-option/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-23-member-access/`。
- 完成条件：B-19/B-20/B-22/B-23 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：待填写。

### [TODO] P7-B2.7：B-33/B-34/B-35 extern、RuntimeError、NoGC/frame boundary contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-33.md`、`audit/strategies/B-34.md`、`audit/strategies/B-35.md`。
- 范围：B-33 8 entries；B-34 6 entries；B-35 5 entries；合计 19 entries。
- 目标：extern global、RuntimeError、try/catch/finally、NoGC/frame slot contract 不再由 codegen 兜底。
- 必须实现：extern global type/store gate、RuntimeError layout verifier、frame/spill slot consistency verifier、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-33-extern-global-funptr/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-34-runtime-error-try/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-35-unsafe-nogc-boundary/`。
- 完成条件：B-33/B-34/B-35 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：待填写。

### [TODO] P7-B2.8：B-08 internal/B-11 member store 与 pure/plain statement route contract

- 参考：`PLAN.md:135-159`、`audit/strategies/B-08.md`、`audit/strategies/B-11.md`。
- 范围：B-08 internal rows 4 entries；B-11 14 entries；合计 18 entries。
- 目标：member store internal rows、pure/plain statement route contract 在 MIR/HIR verifier 后不可达。
- 必须实现：member store receiver/place/value invariant、pure/plain statement boundary verifier、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-08-member-store/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-11-pure-boundary/`。
- 完成条件：B-08 internal rows 与 B-11 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：待填写。

### [TODO] P7-B3.1：B-32/B-31 print/panic/sysroot 与 scalar methods contract

- 参考：`PLAN.md:161-181`、`audit/strategies/B-32.md`、`audit/strategies/B-31.md`。
- 范围：B-32 10 entries；B-31 12 entries；合计 22 entries。
- 目标：sysroot print/panic 桥接与 Float/Int/Char/Bool/String 扩展方法签名稳定。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-32-print-panic-sysroot/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-31-scalar-methods/`。
- 完成条件：B-32/B-31 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：待填写。

### [TODO] P7-B3.2：B-28/B-27 thread/sync intrinsic contract

- 参考：`PLAN.md:161-181`、`audit/strategies/B-28.md`、`audit/strategies/B-27.md`。
- 范围：B-28 20 entries；B-27 58 entries；合计 78 entries。
- 目标：thread/sync intrinsic receiver、arity、return type、destroy/create contract 闭合。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-28-thread-intrinsics/`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-27-sync-intrinsics/`。
- 完成条件：B-28/B-27 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：待填写。

### [TODO] P7-B3.3：B-26 atomic intrinsic contract

- 参考：`PLAN.md:161-181`、`audit/strategies/B-26.md`。
- 范围：B-26，102 entries。
- 目标：atomic intrinsic target mutability、width、ordering、return contract 闭合。
- 必须实现：atomicInt/atomicRef family 的集中签名和 receiver contract，用户错误走 frontend/typecheck，sysroot shape 走 internal sentinel。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-26-atomic-intrinsics/`。
- 完成条件：B-26 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：待填写。

### [TODO] P7-B3.4：B-29 GC intrinsic contract

- 参考：`PLAN.md:161-181`、`audit/strategies/B-29.md`。
- 范围：B-29，93 entries。
- 目标：GC.handleNew/handleGet/handleDrop/pin/unpin 类型和 frame contract 闭合。
- 必须实现：GC intrinsic signature/receiver/return contract、frame reload/pin invariants、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-29-gc-intrinsics/`、`cargo test -p scoop_runtime -- --nocapture`。
- 完成条件：B-29 active count 为 0。
- 依赖：P7-B1 推荐完成后。
- 完成记录：待填写。

### [TODO] P7-B3.5：B-30 named/unsafe/FunPtr/stackmap intrinsic contract

- 参考：`PLAN.md:161-181`、`audit/strategies/B-30.md`。
- 范围：B-30，117 entries。
- 目标：named intrinsic、unsafe/FunPtr、stackmap statepoint contract 闭合。
- 必须实现：FunPtr signature gate、uintPtr/funptr conversion gate、stackmap/statepoint caller/value contract、codegen fallback retire。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-30-named-unsafe-funptr/`、`cargo run -p scoop -- test tests/fixtures/unsafe_nogc/`。
- 完成条件：B-30 active count 为 0。
- 依赖：P7-B1 推荐完成后；B-35 完成后更安全。
- 完成记录：待填写。

## P7-C：RealImpl 退场（203 entries）

### [TODO] P7-C1：B-24 Reflection / comptime intrinsic 实现

- 参考：`PLAN.md:183-206`、`audit/strategies/B-24.md`。
- 范围：B-24，6 entries，`RealImpl`。
- 目标：实现 `sizeOf`、`kindOf`、`descOf`、comptime metadata 参数/返回 contract。
- 必须实现：合法 positive fixture 走真实 codegen；移除对应 `IGNORE-UNTIL-FIX:B-24`；更新 `_index.csv` status。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-24-reflection-comptime/`。
- 完成条件：B-24 active count 为 0；B-24 positive fixture active 并通过。
- 依赖：P7-0-T02；相关 B 类 verifier 完成后更安全。
- 完成记录：待填写。

### [TODO] P7-C2：B-25 Platform / RTTI intrinsic 实现

- 参考：`PLAN.md:183-206`、`audit/strategies/B-25.md`。
- 范围：B-25，14 entries，`RealImpl`。
- 目标：runtime type descriptor、runtime type check metadata、`as?` target runtime type 支持真实 codegen。
- 必须实现：RTTI metadata materialization、runtime type operand/target lowering、fixture active。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-25-platform-rtti/`。
- 完成条件：B-25 active count 为 0；B-25 positive fixture active 并通过。
- 依赖：P7-C1 推荐完成后；B-14/B-22/B-23 contract 稳定后更安全。
- 完成记录：待填写。

### [TODO] P7-C3：B-13 数组 / 复合 transport metadata 实现

- 参考：`PLAN.md:183-206`、`audit/strategies/B-13.md`。
- 范围：B-13，24 entries，`RealImpl`。
- 目标：task transport tuple、resume payload、composed call replay block、array metadata 支持真实 codegen。
- 必须实现：transport tuple metadata、resume payload lowering、composed call replay lowering、fixture active。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-13-composite-transport/`。
- 完成条件：B-13 active count 为 0；B-13 positive fixture active 并通过。
- 依赖：B-03/B-09/B-14 contract 稳定后更安全。
- 完成记录：待填写。

### [TODO] P7-C4：B-12 Closure / lambda / capture 实现

- 参考：`PLAN.md:183-206`、`audit/strategies/B-12.md`。
- 范围：B-12，50 entries，`RealImpl`。
- 目标：closure env、mutable capture、non-scalar capture、callable lookup、lambda return 支持真实 codegen。
- 必须实现：closure env layout、capture materialization、non-scalar transport、callable lookup、lambda return lowering、fixture active。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-12-closure-capture/`、`cargo run -p scoop -- test tests/fixtures/run-pass/`。
- 完成条件：B-12 active count 为 0；B-12 positive fixture active 并通过。
- 依赖：B-03/B-09/B-13 contract 稳定后更安全。
- 完成记录：待填写。

### [TODO] P7-C5：B-10 Effect-typed callable adapter / ABI routing 实现

- 参考：`PLAN.md:183-206`、`audit/strategies/B-10.md`。
- 范围：B-10，109 entries，`RealImpl`。
- 目标：effect callable adapter、continuation carrier、resume token、effect outcome slot、surface function ABI 支持真实 codegen。
- 必须实现：effect callable adapter、continuation carrier lowering、resume token plumbing、effect outcome ABI slot、surface function adapter、fixture active。
- 验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-10-effect-callable-adapter/`、`cargo run -p scoop -- test tests/fixtures/effect_lowered/`、`cargo run -p scoop -- test`。
- 完成条件：B-10 active count 为 0；B-10 positive fixture active 并通过；`tests/fixtures/umb_fix/**` 不再有 `IGNORE-UNTIL-FIX`。
- 依赖：P7-C1、P7-C2、P7-C3、P7-C4；B-03/B-09/B-13 contract 稳定后再推进。
- 完成记录：待填写。

## P8：最终退场

### [TODO] P8-T01：删除 `LlvmEmitError::UnsupportedMainBody` enum variant

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
- 完成记录：待填写。

### [TODO] P8-T02：归档 audit ledger 并更新 DONE 记录

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
- 完成记录：待填写。

## 阶段性统计目标

| 阶段 | Active 目标 | Retired 目标 | 备注 |
|---|---:|---:|---|
| 当前 | 1,284 | 0 | P7-0 前 baseline |
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
