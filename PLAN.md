# UnsupportedMainBody Production 修复计划（P7/P8）

> 生成时间：2026-05-18
> 阶段定位：P7 production 修复 + P8 退场审计
> 上一阶段档案：[`PLAN-1.md`](./PLAN-1.md)、[`TODO-1.md`](./TODO-1.md)
> 设计与 baseline：[`UnsupportedMainBody_FIX.md`](./UnsupportedMainBody_FIX.md) 已完成 P1-P6；`audit/UMB_inventory.csv` 当前 1,284 条。
> 当前状态：待开始。

## 0. 范围与硬约束

- 本计划是实际修复计划，允许修改 production compiler/codegen、frontend/typecheck/HIR/MIR、fixture runner、audit tooling 和相关文档。
- 本计划的唯一目标是让 `LlvmEmitError::UnsupportedMainBody` 从 production 代码路径中退场，最终在 P8 删除该 enum variant。
- 不允许新增 `UnsupportedMainBody { kind: ... }` 站点；不允许把问题改名为另一个后端兜底 diagnostic。
- 每次修复必须以 `audit/UMB_inventory.csv` 中的 `UMB-NNNN` 为最小对账单位，明确本次 retire 哪些 ID。
- 每次 retire 必须同步更新 inventory、bucket 文档、fixture coverage、stale count baseline 和相关测试；不能只改 production 代码。
- `FrontendReject`、`InternalBugSentinel`、`RealImpl` 三类治理路径不能混用：非法用户输入走稳定前端诊断，内部不变量走 verifier/expect/unreachable，合法但未实现的语言特性必须真正实现。
- D 类 `spec uncovered` 当前按第一阶段基线走 `FrontendReject` 治理；不在本计划中顺手扩展 async/generator/yield 等未定义 spec surface。

## 1. 当前基线

数据来源：`cargo run -p scoopc --bin umb-audit -- stats`。

| 指标 | 数值 |
|---|---:|
| Active inventory entries | 1,284 |
| `FrontendReject` | 125 |
| `InternalBugSentinel` | 956 |
| `RealImpl` | 203 |
| Bucket 数 | 36 |
| 缺失 `spec_anchor` | 0 |
| 缺失 `upstream_gate` | 0 |

按治理波次汇总：

| 波次 | 范围 | Entry 数 | 目标动作 |
|---|---|---:|---|
| P7-0 | audit/tooling 稳定化 | 0 | 让 inventory 支持“递减退场”而不重排 ID |
| P7-A | `FrontendReject` | 125 | 上游早拒 + 删除 codegen 兜底 |
| P7-B | `InternalBugSentinel` | 956 | verifier/helper 契约 + `expect`/`unreachable!` |
| P7-C | `RealImpl` | 203 | 补齐合法路径实现并激活 fixture |
| P8 | 退场审计 | 0 | 删除 enum variant、归档 audit ledger |

## 2. P7-0：先修正退场用 audit 机制

第一阶段的 inventory 适合冻结 baseline，但 P7 删除源码站点后会触发两个问题：当前 `inventory_entries()` 按扫描顺序重新生成连续 `UMB-NNNN`，删除早期 row 会导致后续 ID 大面积重排；当前 `EXPECTED_ENTRY_COUNT = 1_284` 也会让 `umb-audit diff` 在真实退场后先失败。正式删除任何 production `UnsupportedMainBody` 前，必须先完成本小阶段。

### P7-0-T01. 引入稳定 ID 与 retired ledger

目标：删除 row 后，未删除的 `UMB-NNNN` 不变。

要求：

- 保留当前 1,284 行作为 immutable baseline，例如 `audit/UMB_inventory_initial.csv` 或等价历史快照。
- 新增 retired ledger，例如 `audit/UMB_retired.csv`，字段至少包含 `id,bucket,expected_class,file,old_line,kind,retired_by,retired_reason,retired_at_notes`。
- 让 active inventory 生成逻辑优先从 baseline/上一版 CSV 继承 ID，而不是按当前扫描顺序重新编号。
- 匹配策略必须能处理 line drift：优先 exact `(file,line,kind)`，其次唯一 `(file,kind,bucket,expected_class,surface)`，最后同组按 old line/source line 顺序配对；无法唯一匹配时必须报错，不能自动重号。
- active IDs 与 retired IDs 必须互斥；二者并集必须等于 initial 1,284 个 ID。

验收：

- `cargo run -p scoopc --bin umb-audit -- diff` 在当前无退场状态仍通过。
- `cargo test -p scoopc audit:: -- --nocapture` 通过。
- 新增或改造测试覆盖“删除一个模拟 row 时 remaining IDs 不重排”的行为。

### P7-0-T02. 把 audit 常量从“冻结总数”改成“退场倒计时”

目标：P7 可以按 PR 递减 active count，而不是永远要求 1,284。

要求：

- 保留 `INITIAL_ENTRY_COUNT = 1_284` 用于 `active + retired == initial`。
- 将当前 active count、literal kind count、dynamic kind count 变成随 active inventory 更新的可维护 baseline。
- `umb-audit stats` 输出 active、retired、initial、by_class、by_bucket。
- `umb-audit diff` 需要报告新增、删除、line drift、field drift，但不能因为 active count 小于 initial 而提前 panic。

验收：

- 当前状态下 active=1,284、retired=0。
- 后续任一修复 PR 只需要更新本次 retired IDs，不会强制全量 fixture `COVERS` 重排。

## 3. 单个修复 PR 的固定流程

每个 production 修复 PR 都按同一套流程执行，避免统计和 fixture 失真。

1. 锁定范围：运行 `cargo run -p scoopc --bin umb-audit -- list --bucket B-XX`，列出本次要 retire 的 ID；必要时再用 `--file PATH` 缩小到单文件。
2. 阅读依据：读取 `audit/UMB_categories/B-XX.md`、`audit/strategies/B-XX.md`、对应 `tests/fixtures/umb_fix/B-XX-*/_README.md` 和 fixture header。
3. 实现修复：按 `expected_class` 做最小 production 改动。
4. 删除兜底：移除对应 `LlvmEmitError::UnsupportedMainBody { ... }` constructor；`InternalBugSentinel` 站点改为命名 helper、`expect` 或 `unreachable!`。
5. 更新 ledger：从 active inventory 移除 retire 的 ID，写入 `audit/UMB_retired.csv`，更新 bucket doc 的 entry count/class distribution。
6. 更新 fixture：移除 retired ID 的 `COVERS` 引用；若 C 类实现完成，去掉对应 fixture 的 `IGNORE-UNTIL-FIX:B-XX`，并把 `_index.csv` status 改为 `active`。
7. 更新 stale count：同步修改 `crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 中对应文件的 `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 与 total。
8. 验证最小集：运行 `cargo test -p scoopc audit:: -- --nocapture`、`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`、`cargo run -p scoopc --bin umb-audit -- stats`、`cargo run -p scoopc --bin umb-audit -- diff`。
9. 验证行为：运行对应 bucket fixture，例如 `cargo run -p scoop -- test tests/fixtures/umb_fix/B-XX-<slug>/`。
10. 合并前全量：阶段性或大 PR 运行 `cargo test --all --all-targets`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`。

## 4. P7-A：FrontendReject 退场（125 entries）

目标：非法用户输入不再进入 LLVM codegen。修复后对应 codegen fallback 要么消失，要么变成 verifier 之后的 impossible branch。

| 优先级 | Bucket | Entry 数 | 计划 |
|---:|---|---:|---|
| A1 | B-16 控制流 outside-of-context | 7 | 最小优先级最高；在 parse/HIR/typecheck 阶段锁定 `break`、`continue`、`return` 上下文错误 |
| A2 | B-08/B-21 成员写入与 struct 字段负例 | 5 | 锁定不可写 target、immutable member store、unknown/missing struct field |
| A3 | B-15 when / 模式匹配用户面 | 55 | 完备性、arm 类型、enum variant、guard 和 payload arity 统一早拒 |
| A4 | B-36 spec uncovered | 58 | async/generator/yield/class/annotation 等未定义 surface 给稳定 frontend diagnostic |

实现规则：

- 错误码与文案必须稳定，且不得包含 `后端`、`backend`、`LLVM`、`codegen`、`UnsupportedMainBody`。
- Negative fixture 需要从“可覆盖 UMB”转为“真实 frontend reject”；对应 `umb_fix` fixture 不应 skip。
- 对已被前端保证不可达的 codegen 分支，删除 `UnsupportedMainBody` 后用具体 invariant 说明保留 `unreachable!` 的原因。

阶段验收：

- `FrontendReject` active inventory 从 125 降为 0。
- B-08/B-15/B-16/B-21/B-36 的 negative fixture 全部 active 并通过。
- `pipeline_user_visible_failure_policy` 中 forbidden terms 测试仍通过。

## 5. P7-B：InternalBugSentinel 退场（956 entries）

目标：把“LLVM 主 codegen 临时兜底”改成上游 contract 或 codegen helper 的明确内部不变量。用户输入不应看见这些错误。

### P7-B1. Helper invariant（B-01，71 entries）

先做 B-01，因为它提供后续迁移的 helper 风格。

要求：

- 引入集中 helper：`expect_insert_block`、`expect_parent_function`、`expect_entry_block`、`expect_basic_value`。
- helper panic 文案带 helper 名称和上下文，不复用 `UnsupportedMainBody` diagnostic。
- B-01 的 sentinel coverage 从 `_README.md`/audit test 转入 retired ledger。

验收：B-01 active inventory 为 0，B-01 sentinel test 仍能证明 helper-only 覆盖闭环。

### P7-B2. Core MIR/HIR/type/layout contract（473 entries）

范围：B-02、B-03、B-04、B-05、B-06、B-07、B-08 internal rows、B-09、B-11、B-14、B-17、B-18、B-19、B-20、B-21 internal rows、B-22、B-23、B-33、B-34、B-35。

推荐顺序：

| 顺序 | Bucket | Entry 数 | 关键 contract |
|---:|---|---:|---|
| B2.1 | B-02/B-04 | 35 | local、param、return type 必须在 MIR 完成物化 |
| B2.2 | B-05 | 25 | CFG start block、target、terminator shape 必须合法 |
| B2.3 | B-06/B-07/B-21 | 80 | aggregate/pattern/field schema 必须在 MIR verifier 通过 |
| B2.4 | B-03/B-09/B-14 | 96 | call ABI、TypeStore equivalence、cast/typecheck contract 闭合 |
| B2.5 | B-17/B-18 | 51 | scalar coercion、literal/string value contract 闭合 |
| B2.6 | B-19/B-20/B-22/B-23 | 149 | top-level/object/class/enum/member layout contract 闭合 |
| B2.7 | B-33/B-34/B-35 | 19 | extern global、RuntimeError、NoGC/frame boundary contract 闭合 |
| B2.8 | B-08 internal/B-11 | 18 | member store internal rows、pure/plain statement route contract 闭合 |

实现规则：

- 优先在 MIR strict verifier 或物化阶段表达 contract，避免在 codegen 每个 use-site 单点 `expect`。
- 如果 contract 是“frontend/typecheck 已保证”，要补对应 verifier assert，防止未来 HIR/MIR bypass 重新打穿。
- `TypeStore` 等价问题优先做集中函数，不能在每个 codegen use-site 局部 fallback。
- 每个 bucket 完成后，对应 `audit/strategies/B-XX.md` 追加 P7 完成记录或 retired pointer。

阶段验收：Core MIR/HIR/type/layout 相关 active inventory 为 0，且 `cargo run -p scoop -- test tests/fixtures/umb_fix/` 全部通过。

### P7-B3. Intrinsic/sysroot contract（412 entries）

范围：B-26、B-27、B-28、B-29、B-30、B-31、B-32。

推荐顺序：

| 顺序 | Bucket | Entry 数 | 关键 contract |
|---:|---|---:|---|
| B3.1 | B-32/B-31 | 22 | print/panic/sysroot 桥接与标量扩展方法签名 |
| B3.2 | B-28/B-27 | 78 | thread/sync intrinsic receiver、arity、return type |
| B3.3 | B-26 | 102 | atomic intrinsic target mutability、width、ordering、return contract |
| B3.4 | B-29 | 93 | GC handle/pin/unpin 类型和 frame contract |
| B3.5 | B-30 | 117 | named/unsafe/FunPtr/stackmap intrinsic contract |

实现规则：

- 每个 intrinsic family 建一个签名/receiver/return contract helper，不能重复散落检查。
- 对用户可写错的 intrinsic 调用，前端或 typecheck 必须给稳定诊断；对 sysroot 内部 shape，走 `InternalBugSentinel`。
- `unsafe`/`NoGC`/stackmap 相关改动必须额外跑 runtime/GC/NoGC fixture。

阶段验收：`InternalBugSentinel` active inventory 从 956 降为 0。

## 6. P7-C：RealImpl 退场（203 entries）

目标：合法语言 surface 走真实 codegen，不再 skip/xfail happy-path fixture。

| 优先级 | Bucket | Entry 数 | 计划 |
|---:|---|---:|---|
| C1 | B-24 Reflection / comptime intrinsic | 6 | 先实现小 surface：`sizeOf`、`kindOf`、`descOf`、comptime metadata 参数和返回 contract |
| C2 | B-25 Platform / RTTI intrinsic | 14 | runtime type descriptor、runtime type check metadata、`as?` target runtime type |
| C3 | B-13 数组 / 复合 transport metadata | 24 | task transport tuple、resume payload、composed call replay block、array metadata |
| C4 | B-12 Closure / lambda / capture | 50 | closure env、mutable capture、non-scalar capture、callable lookup、lambda return |
| C5 | B-10 Effect-typed callable adapter / ABI routing | 109 | effect callable adapter、continuation carrier、resume token、effect outcome slot、surface function ABI |

实现规则：

- C 类不能通过 frontend reject 规避；只要 spec 已定义且 fixture 是 positive，就必须生成有效 LLVM/可运行输出。
- 每完成一个 C bucket，移除对应 fixture 的 `IGNORE-UNTIL-FIX:B-XX`，并将 `_index.csv` status 改为 `active`。
- B-10 依赖调用 ABI、transport metadata、closure callable lookup；除非 C1-C4 已稳定，否则不要先大改 B-10。
- 如果实现过程中发现 spec 确实未定义，停止该 bucket，更新 `audit/strategies/B-XX.md` 为 blocked-on-spec，不允许临时 fallback。

阶段验收：

- `RealImpl` active inventory 从 203 降为 0。
- `tests/fixtures/umb_fix/**` 不再有 `IGNORE-UNTIL-FIX`。
- 对应 run-pass/codegen/effect fixture 全部 active 并通过。

## 7. P8：最终退场

触发条件：`cargo run -p scoopc --bin umb-audit -- stats` 显示 active inventory 为 0，retired inventory 为 1,284。

一次性动作：

1. 从 `crates/scoopc/src/llvm/mod.rs` 删除 `LlvmEmitError::UnsupportedMainBody` 变体和相关 diagnostic 映射。
2. 删除或归档 `crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 中 `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 的历史计数；最终 total 必须为 0。
3. 将 `audit/UMB_inventory_initial.csv`、`audit/UMB_retired.csv`、最终 empty inventory 和本计划归档到 `docs/archive/`。
4. 删除或改造只服务于 UMB 退场的 audit tests/bin；保留有长期价值的 fixture coverage 测试。
5. 更新 `UnsupportedMainBody_DONE.md`，记录 P8 完成时间、最终验证命令和归档位置。

最终验收：

- `rg -n "UnsupportedMainBody" crates/scoopc/src/llvm` 不再命中 production codegen 路径。
- `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过，且 stale unsupported total 为 0 或该历史测试已归档。
- `cargo run -p scoop -- test tests/fixtures/umb_fix/` 通过且无 ignored/xfail 状态。
- `cargo test --all --all-targets`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings` 通过。

## 8. PR 切分建议

- PR-0：只做 P7-0 audit/tooling 稳定化，不退场 production row。
- PR-A1 到 PR-A4：按 P7-A 顺序处理 frontend reject，每个 PR 退场一个小组。
- PR-B1：B-01 helper invariant。
- PR-B2 系列：Core MIR/HIR/type/layout contract，按 B2.1 到 B2.8 分 PR；每个 PR 控制在约 20 到 100 个 retired IDs。
- PR-B3 系列：Intrinsic/sysroot contract，按 intrinsic family 分 PR；B-26/B-29/B-30 可继续按文件或 API 子族拆分。
- PR-C 系列：RealImpl 从小到大推进；B-10 单独拆成 callable adapter、continuation carrier、resume token、effect outcome ABI 几个 PR。
- PR-P8：只做最终删除、归档和退场审计，不混入功能修复。

## 9. 完成判据

- Active `audit/UMB_inventory.csv` 行数为 0。
- Retired ledger 覆盖全部 1,284 个 initial `UMB-NNNN`，无重复、无缺失。
- `FrontendReject`、`InternalBugSentinel`、`RealImpl` 三类 active count 全为 0。
- `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 总数为 0 或已随 P8 归档删除。
- 所有 `umb_fix` fixture active 且通过。
- `LlvmEmitError::UnsupportedMainBody` enum variant 已删除。
- 全量验证命令通过，并在 `UnsupportedMainBody_DONE.md` 留下最终记录。
