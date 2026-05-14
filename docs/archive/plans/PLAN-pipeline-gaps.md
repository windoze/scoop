# Scoop：新主线收口与旧主线清理计划

> 生成时间：2026-05-14  
> 差距基线：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md)  
> 格式参考：`docs/archive/plans/PLAN-stable-id.md`、`docs/archive/plans/PLAN-effect-refactor.md`  
> 本轮主题：按 `PIPELINE_GAPS.md` 当前账本，把默认/refactor 新主线上的全部 live gap 收口，同时把旧主线 residual producer / fallback / legacy-only guard 从可执行代码、inventory、fixture 与验证路径中彻底清除。  
> 重要边界：`Closed/Re-scoped`、`Historical`、`FrontendReject` 不是“顺手打开”的 backlog；除非本计划显式要求，否则它们的目标是保持边界稳定、reject 明确、实现与文档一致。  
> 行号说明：下文引用以本计划生成时的路径和函数名为准；若后续行号漂移，优先按文件路径、reason 字符串和测试名定位。

## 0. 工作原则

- `PIPELINE_GAPS.md` 是本轮唯一 gap 基线。若实现过程中要改变某个 gap 的归类、owner 或收口方式，必须先回写 `PIPELINE_GAPS.md`，再继续实现。
- 整个编译 pipeline 的最终用户可见 contract 只有两类结果：
1. 输入合法且有效，则编译器必须产生正确输出。
2. 输入非法，则编译器必须返回明确、稳定、可定位到源码的错误信息。
- 除上述两类结果外，其余一切行为都视为编译器 bug，包括但不限于：panic、assertion、`UnsupportedMainBody`、late unsupported bucket、静默 default-value fallback、误编译，以及“尚未支持某特性”式的模糊兜底。
- 本轮的完成目标不是“再做一版并行新线”，而是把当前默认/refactor 主线直接收口到 production 完整状态。
- 本轮的完成目标也不是“让 legacy 代码不可达就算完”。凡是只为旧主线存在的 producer、fallback、guard、reason string、测试白名单、inventory bucket，都必须从 active code 中删除。
- `LegacyOnly` 清理范围不只限于 `PIPELINE_GAPS.md` 明面上的条目。凡是语义上只服务旧主线、只是在当前账本里尚未单列成 `LegacyOnly` 的 residual branch，也属于删除范围。
- `Open` 和影响默认主线的 `Partial` 必须在本轮归零。最终允许保留的非关闭状态只有显式 `FrontendReject` 和文档级 `Historical`。
- `Partial` 的收口方式只有两种：
1. 把当前默认主线已接受的 surface 真正补齐到 production 级实现。
2. 若该 surface 不属于默认主线，应把接受面前移为显式 gate，并把 gap 改写为 `Closed/Re-scoped` 或 `FrontendReject`。
- backend 可以保留最终 guard，但 guard 只能表达“新主线 contract 被破坏”或“不应到达的 impossible state”；不得继续复用旧主线 reason string 充当业务分支。
- `FrontendReject` 只表示“该输入在当前语言 contract 下非法，必须以前端明确诊断拒绝”；它不是“后端还没实现所以先报 unsupported”的同义词。
- raw MIR、effect-lowered LLVM、runtime C 只允许消费显式 handoff contract、facts、layout metadata 和 target/session config；不得回看旧 HIR/MIR fallback 语义来补洞。
- `PIPELINE_GAPS.md` 里的 legacy gap id 可以继续作为文档历史映射保留，但 executable inventory 不得在本轮结束后继续保留 `LegacyOnly` disposition 作为活代码分类。
- 本轮的验证必须同时证明两件事：
1. 新主线 live gap 已收口。
2. 旧主线 residual code 已从 active tree 中消失，而不是只是躲到“理论上不可达”的角落。
- 本轮的验证还必须证明：生产路径上不再把 `Unsupported*`、`Todo`、late fallback、panic/assertion 当成合法的用户可见结果；它们只能作为内部 bug sentinel 或测试期 impossible-state guard 存在。

## 1. 当前判断

- 当前 live gap 已经被 `PIPELINE_GAPS.md` 收敛成四个主簇：
1. pre-MIR / MIR handoff 仍有 placeholder、contract drift 和 materialization strictness 缺口。
2. raw MIR route 仍有 effect/control terminator、`PerformResult`、dynamic call kind 和 member continuation route 缺口。
3. effect-refactor 主线仍有 actual outward effect routing、effect-typed callable adapter、cleanup/unwind、`main(args)` ABI routing 缺口。
4. aggregate/composite transport 仍集中缺在 enum payload、array composite element、closure env / composite boxing 边角。
- 旧主线残留不只出现在 `§1.6-§1.9`。当前 `mir` inventory 还把 `sizeOf` / `nameOf` 的旧 fallback 记成 `LegacyOnly`，producer 仍在 `crates/scoopc/src/mir/lower.rs`，而文档主条目 `§6.3` 已经标成 `Closed/Re-scoped`。这说明“Legacy 清理范围”已经超过文档显式 `LegacyOnly` 小节。
- executable inventory 目前把 live gap、下游 guard、closed id、legacy-only residue 混在一起维护。`mir/placeholder_inventory.rs` 仍保留 `LegacyOnly` disposition，`llvm/codegen_gap_inventory.rs` 也仍把部分已 `Closed/Re-scoped` 的编号作为 production-blocker map 的组成部分。计划必须先把这层边界理顺，否则后续很难判断“是在修 live gap，还是在维护历史编号”。
- 当前最大的实现风险不是少一两个 LLVM helper，而是旧 fallback 仍在掩盖 contract 漏洞。只要 `mir/lower.rs` 里的旧 assign/call/dispatch/resume/intrinsic fallback 还在，很多“应该更早 fail-fast”的 bug 就会继续被转写成 Todo/unsupported 桶。
- 当前还需要进一步统一的一点是“非法输入”和“编译器 bug”的边界：凡是 parser/typecheck/HIR/MIR contract 已经接受为合法程序的输入，后续阶段若仍落到 `Unsupported*`、assertion 或默认值兜底，就必须按编译器 bug 处理，而不是继续归类成“尚未支持”。
- `PIPELINE_GAPS.md §3.7` 之类已关闭项不应再作为 backlog owner；它们只应保留 regression audit 责任，防止 raw MIR 重新发射未规范化函数引用。

## 2. Gap 覆盖矩阵

| Gap | 当前状态 | 本轮动作 | 归属阶段 |
|---|---|---|---|
| `§1.1` `comptime` block/if/for | Open | 在进入 runtime MIR 前展开或诊断 | P2 |
| `§1.4` top-level `val` | Open | 建立 MIR root / initializer model | P2 |
| `§1.6` assign LHS legacy fallback | LegacyOnly | 删除 producer、guard、inventory、fixture 白名单 | P1 |
| `§1.7` call/ctor provenance legacy fallback | LegacyOnly | 删除 producer、guard、inventory、fixture 白名单 | P1 |
| `§1.8` dispatch callee legacy fallback | LegacyOnly | 删除 producer、guard、inventory、fixture 白名单 | P1 |
| `§1.9` `Continuation.resume` legacy fallback | LegacyOnly | 删除 producer、guard、inventory、fixture 白名单 | P1 |
| `§2.1` strict MIR validator | Open | strict no-placeholder / no-sentinel verifier | P2 |
| `§2.3` raw MIR Todo guard | Open | 保留 impossible-state guard，但 production MIR 不可达 | P2 |
| `§2.4` `Return { value: None }` drift | Open | verifier 拒绝或 MIR 显式改写 | P2 |
| `§2.5` generic template / root missing | Open | materializer hard error 与 root index 完整化 | P2 |
| `§2.7` `TypeKind::Param` 到达 codegen | Partial | 彻底消除 concrete path 上的 param drift，保留最终 guard | P2 |
| `§3.1` unsupported raw MIR terminator | Open | route verifier 或完整 lowering | P3 |
| `§3.2` raw MIR `Perform` cleanup/resume_target | Open | route verifier 或完整 lowering | P3 |
| `§3.3` raw MIR `PerformResult` default value | Open | 删除 default-value path，改成真实 lowering 或 upstream reject | P3 |
| `§3.5` effect-neutral cast/typecheck narrow residual | Partial | 补齐受支持子集或把剩余 surface 前移 gate | P6 |
| `§3.6` raw MIR `Virtual` / `Interface` / `Resume` call kind | Open | route verifier 或完整 lowering | P3 |
| `§3.8` MIR pattern `is Type` narrow residual | Partial | 扩大到默认主线支持面或前移 gate | P5 |
| `§3.9` ctor named/default arg contract drift | Partial | 完整 selected ctor + ordered args contract | P3 |
| `§3.10` default arg canonicalization drift | Partial | upstream complete binding，backend 不再补齐 | P3 |
| `§3.11` closure env / capture shape | Partial | 完整 env transport contract 或前移 gate | P5 |
| `§3.12` effect-typed callable adapter | Open | 实现 plain/effect adapter 与 callable routing | P4 |
| `§3.13` `StoreMember` continuation route ambiguous | Open | upstream resolve/reject `Ambiguous` | P3 |
| `§4.1` aggregate boxing residual | Partial | 随 composite transport 统一后关闭 | P5 |
| `§4.3` large integer enum payload | Open | 统一 boxed/transport layout | P5 |
| `§4.4` nested enum/tuple/struct payload | Open | 统一 boxed/transport layout | P5 |
| `§4.5` array composite element transport | Open | 完整 metadata + composite get/set path | P5 |
| `§5.1` actual outward effect routing | Open | ABI 由 actual outward effect set 决定 | P4 |
| `§5.3` cleanup/unwind contract | Partial | 完整 cleanup/unwind semantic contract | P4 |
| `§5.4` outward-empty callable misroute | Open | outward-empty 强制 plain ABI，修 `main(args)` | P4 |
| `§6.3` residual legacy intrinsic fallback | Closed/Re-scoped | 删除旧 `sizeOf` / `nameOf` fallback 残留 | P1 |
| `§7.1` or-pattern binder | FrontendReject | 保持 gate，与 backend 能力同步 | P6 |
| `§7.2` function type runtime cast | FrontendReject | 保持 gate 或在单独特性计划中重开 | P6 |
| `§7.3` use-site effect row type arg | Closed/Re-scoped | 名义类型上的 `Type<eff Row>` 已支持；非法 target 维持前端诊断 | P6 |
| `§7.5` struct mutable field | FrontendReject | 保持 gate，避免 value-type store drift | P6 |
| `§7.6` GC pin/handle intrinsic narrow residual | Partial | 收口支持子集或前移 gate | P6 |

## 3. 代码入口总表

| 主题 | 入口文件 / 位置 | 当前问题 | 目标状态 |
|---|---|---|---|
| legacy producer 清理 | `crates/scoopc/src/mir/lower.rs`、`crates/scoopc/src/mir/placeholder_inventory.rs`、`crates/scoopc/src/pipeline/hir_preflight.rs`、`crates/scoopc/src/pipeline/mir_stage.rs`、`crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/materialize.rs` | 旧 assign/call/dispatch/resume/intrinsic fallback 仍在 producer、guard、测试和 inventory 中留痕 | 删除所有 legacy-only executable path；历史 id 只留文档 |
| pre-MIR / MIR handoff | `crates/scoopc/src/pipeline/hir_preflight.rs`、`crates/scoopc/src/pipeline/hir_stage.rs`、`crates/scoopc/src/pipeline/mir_stage.rs`、`crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/materialize.rs` | `comptime_*`、top-level root、`unterminated`、`Return None`、generic root drift 仍会漏到下游 | strict production MIR 与 materialized MIR handoff |
| raw MIR LLVM route | `crates/scoopc/src/llvm/codegen/mir_body.rs` | effect/control terminator、`PerformResult`、unsupported call kind 和 default arg drift 仍晚到 codegen 才炸 | raw MIR 只接收受支持输入；其余 upstream reject 或 route 到 late-lowered boundary |
| effect ABI / adapter / unwind | `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`、`crates/scoopc/src/llvm/codegen/mod.rs` | callable ABI 仍可能按内部 shape 分类；effect-typed callable adapter、cleanup/unwind、`main(args)` 路由未闭合 | actual outward effect set 唯一决定 ABI，plain/effect adapter 完整 |
| aggregate / composite transport | `crates/scoopc/src/llvm/codegen/enum_lowering.rs`、`crates/scoopc/src/llvm/codegen/control_flow.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`、`crates/scoopc/src/llvm/codegen/mir_body.rs` | enum payload、array composite element、closure env / boxing 仍各走一套特殊规则 | 统一 composite transport / boxing / layout contract |
| frontend gate 同步 | `crates/scoopc/src/pipeline/mir_stage.rs`、`crates/scoopc/src/typecheck/lower.rs`、`crates/scoopc/src/typecheck/when_pat.rs`、`crates/scoopc/src/typecheck/structs.rs`、`crates/scoopc/src/typecheck/expr/error.rs` | `FrontendReject` 与 backend 能力、partial surface 边界未完全一致 | 所有默认主线外 surface 都以前端 gate 明确阻断 |
| 文档 / inventory / fixture 审计 | `PIPELINE_GAPS.md`、`crates/scoopc/src/llvm/codegen_gap_inventory.rs`、`crates/scoopc/src/llvm/tests.rs`、`tests/fixtures/**` | live gap、historical id、legacy cleanup、closed blockers 混在同一套验证里 | docs 记录历史，inventory 记录活跃 contract，fixture 覆盖最终主线 |

## 4. 顺序总览

1. P0：冻结范围、分类规则与 executable inventory 边界。
2. P1：删除旧主线 residual producer、legacy-only guard 与可执行 inventory bucket。
3. P2：收紧 pre-MIR / MIR handoff，关闭 placeholder、top-level root 和 materialization contract 漏洞。
4. P3：收口 raw MIR route 与 call/ctor/member continuation contract。
5. P4：收口 effect-refactor ABI、adapter、cleanup/unwind 与 outward-empty routing。
6. P5：统一 aggregate/composite transport，关闭 enum/array/closure/boxing 残余缺口。
7. P6：同步 frontend gates、收尾 partial surface、重写 gap 分类到最终状态。
8. P7：执行 full regression、grep 审计和阶段退出复核。

依赖说明：

- P1 必须早于后续实现期的大部分 backend 收口，因为 legacy producer 会掩盖 contract 漏洞。
- P2 必须早于 P3-P5，因为 raw MIR / LLVM 不应继续替上游修 `Todo`、`Return None`、`TypeKind::Param` 和 root drift。
- P3 必须早于 P4，因为 callable ABI routing 之前先要明确 raw MIR route 的合法输入集合。
- P4 与 P5 可以局部交错推进，但 P4 的 effect payload / cleanup contract 依赖 P5 提供统一 composite transport。
- P6 只能在 P1-P5 基本收口后进行，否则很容易把真正的 live gap 误写成 `FrontendReject` 或 `Closed/Re-scoped`。
- P7 之前不算完成；只做定向单测和少量 fixture 通过不等于主线已闭合。

## 5. 分阶段计划

### P0. 冻结范围、分类规则与 executable inventory 边界

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §0、§8、§9。

目标：

- 把“live gap / downstream guard / legacy residual / historical id / frontend gate” 的边界先定死。
- 给后续删除旧主线代码和关闭新主线 gap 提供同一份 owner map。
- 明确本轮结束时哪些状态允许继续存在，哪些必须归零。

必须实现的内容：

1. 复核 `crates/scoopc/src/mir/placeholder_inventory.rs`、`crates/scoopc/src/hir/lower/placeholder_inventory.rs`、`crates/scoopc/src/llvm/codegen_gap_inventory.rs` 的分类边界，明确哪些条目是 live contract，哪些只是文档遗留映射。
2. 建立一份实现期常驻审计清单，至少覆盖以下活跃搜索域：
   `crates/scoopc/src`、`crates/scoop/src`、`tests/fixtures`。
3. 固定旧主线 residual 审计词表，至少覆盖：
   `assign lhs missing local`、`assign lhs lowering pending`、`call callee lowering pending`、`ctor call lowering pending`、`sizeOf intrinsic requires value or type arg`、`nameOf intrinsic requires type arg`、`resume lowering requires canonical callee shape`、`dispatch callee lowering pending`。
4. 固定本轮退出条件：
   `Open = 0`、默认主线相关 `Partial = 0`、active code 中 `LegacyOnly = 0`。
5. 明确 `Historical` / `Closed/Re-scoped` / `FrontendReject` 的处理策略：
   文档可保留编号，active code 不再把它们当默认 blocker；若仍需 executable guard，guard 语义必须改写为当前 contract violation，而不是旧 gap 名称。
6. 固定“非法输入 vs 编译器 bug”边界：
   parser/typecheck/HIR/MIR 已接受的输入若在后续阶段失败，一律按编译器 bug 审计；只有被更早阶段显式诊断拒绝的输入，才算非法输入。

必须遵从的约束：

- P0 只冻结边界，不提前把 live gap 人为改成 `Historical` 或 `FrontendReject`。
- P0 不得用“inventory 里先删条目”代替真实实现或真实删除。
- 若某个 gap 同时涉及 live implementation 和 legacy residual，必须在后续阶段同时安排“补能力”和“删旧代码”两个动作，不能只做其一。

阶段输出：

- 一份稳定的 gap-to-phase owner map。
- 一份 legacy reason 审计词表。
- 一份明确的状态收口规则。

验证：

1. `cargo test -p scoopc refactor_hir_placeholder_inventory`
2. `cargo test -p scoopc refactor_mir_placeholder_inventory`
3. `cargo test -p scoopc codegen_gap_inventory`
4. 对活跃代码树执行 `LegacyOnly` 和上述 legacy reason 字符串搜索，记录基线命中。

完成条件：

- 后续每个活跃 gap 都有唯一执行阶段。
- “旧主线代码删除”已经被定义成可验证的工程动作，而不是口头原则。

### P1. 删除旧主线 residual producer、legacy-only guard 与可执行 inventory bucket

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.6、§1.7、§1.8、§1.9、§6.3。

目标：

- 把旧主线 residual producer 从 MIR lowering 和相关测试/guard 中彻底移除。
- 让新主线 contract 缺口暴露为真实 diagnostic / verifier failure，而不是被旧 fallback 转写成 Todo。

必须实现的内容：

1. 删除 `crates/scoopc/src/mir/lower.rs` 中所有只为旧主线存在的 fallback producer，至少包括：
   assign LHS fallback、call/ctor provenance fallback、`sizeOf` / `nameOf` fallback、legacy resume fallback、legacy dispatch fallback。
2. 删除或改写 `crates/scoopc/src/pipeline/hir_preflight.rs`、`crates/scoopc/src/pipeline/mir_stage.rs`、`crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/materialize.rs` 中与上述 legacy reason 绑定的白名单、负例、forbidden list 和 guard。
3. 在最后一个 legacy producer 删除后，移除 executable inventory 中的 `LegacyOnly` disposition 和对应 entries；若需要保留 legacy id，对应 mapping 只留在 `PIPELINE_GAPS.md` 或 archive 文档。
4. 审计本阶段触及的 `uses_refactor_typed_contracts()` 或等价分叉点。凡是该分叉只为旧主线存在，必须顺手删除，而不是留下 dormant branch。
5. 为替代 legacy fallback 的真实 contract failure 补充清晰 diagnostic 或 strict verifier，避免删除旧代码后错误只剩 panic 或模糊 `UnsupportedMainBody`。
6. 将 `§6.3` 下已经 closed 的 runtime reflection intrinsic residual fallback 一并清理，防止文档与实现长期分裂。

必须遵从的约束：

- P1 的目标是“删除旧代码”，不是把旧 reason string 换成另一句同义 Todo。
- P1 不得把 legacy fallback 装成“暂时不可达的 if 分支”保留在活代码里。
- 若删除 legacy producer 暴露出新主线 contract 漏洞，该漏洞必须在 P2-P6 被正面修复，不得通过恢复 fallback 解决。
- P1 删除 legacy fallback 后若暴露出 panic/assertion/unsupported，只能说明真实 bug 被显露出来；不得以“功能暂不支持”重新包装。

阶段输出：

- `mir` lowering 不再含旧主线 residual producer。
- active inventory 不再有 `LegacyOnly` bucket。
- legacy reason 不再出现在主干 verifier、fixture 白名单和 MIR dump forbidden list 中。

验证：

1. `cargo test -p scoopc refactor_mir_place_contract`
2. `cargo test -p scoopc refactor_mir_call_contract`
3. `cargo test -p scoopc refactor_mir_placeholder_inventory`
4. 对以下搜索点执行审计，确认命中只剩文档或 archive：
   `assign lhs missing local`
   `assign lhs lowering pending`
   `call callee lowering pending`
   `ctor call lowering pending`
   `sizeOf intrinsic requires value or type arg`
   `nameOf intrinsic requires type arg`
   `resume lowering requires canonical callee shape`
   `dispatch callee lowering pending`
   `LegacyOnly`

完成条件：

- 旧主线 residual code 不再存在于 active compiler path。
- 删除这些代码后，任何失败都已经转化为新主线 contract diagnostic 或后续 live gap owner。

### P2. 收紧 pre-MIR / MIR handoff，关闭 placeholder、top-level root 与 materialization drift

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.1、§1.4、§2.1、§2.3、§2.4、§2.5、§2.7。

目标：

- 让 production MIR 和 materialized MIR 成为真正可下游信任的 handoff。
- 把 `Todo`、`Return { value: None }`、missing root、`TypeKind::Param` 等错误前移到 verifier / materializer。
- 消除 top-level root 和 comptime expansion 的前置缺口。

必须实现的内容：

1. 在进入 runtime MIR 之前完成 `comptime block`、`comptime if`、`comptime for` 的展开或显式诊断，禁止这些 surface 继续以 placeholder 漂到 MIR。
2. 为 top-level `val` 建立 MIR declaration / initializer root，而不是继续生成 `Item::Todo`。
3. 把 `unterminated` sentinel 从“允许通过 direct-style validator”改为 production hard failure。
4. 统一 `Return { value: None }` contract：
   非 `Unit` 返回不得再由 raw MIR codegen 合成默认值；要么在 MIR 中显式写出返回值，要么 verifier 直接拒绝。
5. 固化 generic template / MIR root 缺失的 materialization hard error，并补齐 root index / instance key 发布路径，避免下游再碰 missing root。
6. 彻底消除 concrete path 上 `TypeKind::Param` 到达 codegen 的可能；最终 codegen guard 只用于 impossible-state 审计，不再充当业务补丁。
7. 保留 `§2.3` 的 downstream Todo guard 作为最终防线，但需要同步改写其语义：
   这些 guard 只能表达“production MIR contract 被破坏”，不能再承担默认主线能力。

必须遵从的约束：

- P2 不得继续依赖 raw MIR codegen 的 default-value、late unsupported 或 assertion bucket 来补齐上游 contract。
- P2 不得把 generic/template drift 静默改成“选一个 fallback FQN 先跑过去”。
- P2 的 strict verifier 必须同时覆盖 stage output 和 materialized output，不能只守住其中一侧。
- P2 结束后，production MIR/materialized MIR 的失败形态只能是明确诊断或内部 bug sentinel；不得再向用户暴露“进入 codegen 后才发现 unsupported”。

阶段输出：

- strict production MIR handoff。
- strict materialized MIR handoff。
- top-level init / generic root / comptime expansion 的闭合 contract。

验证：

1. `cargo test -p scoopc refactor_mir_placeholder_inventory`
2. `cargo test -p scoopc codegen_gap_inventory`
3. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor`
4. 对 `PIPELINE_GAPS.md §9` 中的 MIR / placeholder / handoff 相关定向单测补跑：
   `cargo test -p scoopc refactor_mir_value_primitives_reject_unsupported_function_type_cast_before_mir`
   `cargo test -p scoopc codegen_gap_inventory`
   `cargo test -p scoopc refactor_llvm_source_classification_verifier`

完成条件：

- production MIR 与 materialized MIR 都不再把 `Todo`、`Return None`、missing root、`TypeKind::Param` 漏给下游。
- `§1.1`、`§1.4`、`§2.1`、`§2.4`、`§2.5`、`§2.7` 可关闭；`§2.3` 只保留为 impossible-state guard。

### P3. 收口 raw MIR route 与 call/ctor/member continuation contract

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.1、§3.2、§3.3、§3.6、§3.9、§3.10、§3.13。

目标：

- 让 raw MIR emitter 的合法输入集合变成显式 contract。
- 让 ctor/default arg/member continuation route 之类语义在 upstream contract 中闭合，不再晚到 LLVM 才猜 shape。
- 删除 `PerformResult` default-value 这类潜在 miscompile 路径。

必须实现的内容：

1. 明确 raw MIR route policy：
   `Handle`、`ResumeUnwind`、raw `Perform`、raw `PerformResult`、`Virtual`、`Interface`、`Resume` call kind 要么拥有完整 lowering，要么在 route verifier 阶段被拒绝并改走 published boundary / late-lowered path。
2. 删除 raw MIR `PerformResult` 的默认值发射路径，禁止任何可能的 silent miscompile。
3. 为 ctor lowering 补齐 authoritative selected ctor + ordered bound args contract，彻底消除 named/default/delegation 仍依赖 backend 猜测的残余。
4. 为默认参数 canonicalization 补齐 upstream binding map，backend 不再承担补齐参数和 arity 容错的职责。
5. 对 `StoreMember` continuation route 的 `Ambiguous` 结果建立 upstream resolve/reject 规则，LLVM 不再承担歧义拆解。
6. 将 `§3.7` 作为 regression audit 保留：
   raw MIR 不得重新直接发射未规范化的普通函数引用；若回归，应在本阶段被 verifier 或 normalization 测试发现。

必须遵从的约束：

- P3 不是“把 raw MIR 变成另一个兜底全能后端”。凡是应该在 upstream contract 决定的语义，必须上移。
- P3 不得继续保留 default-value、arity-mismatch 自动补洞或 string-based owner/member 恢复。
- 若某类 call-like surface 最终必须走 late-lowered/effect path，则 raw route 应明确拒绝，而不是半支持。
- P3 中 raw route verifier 的失败必须可追溯到上游 contract bug 或更早诊断缺失，而不是以用户可见“尚未支持的 raw MIR 形状”结束。

阶段输出：

- raw MIR 合法输入集合与 route policy。
- 完整的 ctor/default arg/member continuation MIR contract。
- `PerformResult` default-value path 清零。

验证：

1. `cargo test -p scoopc codegen_gap_inventory`
2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
3. `cargo test -p scoopc llvm_tests`
4. 回归时重点关注 `class_ctor_named_default_and_delegation_basic.scoop` 与 `top_level_callable_value_call_basic.scoop`。

完成条件：

- `§3.1`、`§3.2`、`§3.3`、`§3.6`、`§3.9`、`§3.10`、`§3.13` 不再是默认主线 live gap。
- raw MIR emitter 不再承担 upstream contract 修复责任。

### P4. 收口 effect-refactor ABI、callable adapter、cleanup/unwind 与 outward-empty routing

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.12、§5.1、§5.3、§5.4。

目标：

- 让 actual outward effect set 成为 callable ABI 的唯一分类依据。
- 让 effect-typed closure/function-value/`FunPtr` surface 在 plain 与 effect ABI 之间有完整 adapter/boundary。
- 让 cleanup/unwind 与 `main(args)` routing 收口到 production contract。

必须实现的内容：

1. 在 `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`、`.../value.rs`、`llvm/codegen/mod.rs` 中统一 callable ABI routing：
   outward-empty callable 一律发布 plain ABI；
   actual-outward 非空或明确 adapter surface 才发布 effect-step / adapter path。
2. 为 plain closure、function-value、`FunPtr` 在 effect-typed surface 上补齐 adapter 或 published boundary lowering，禁止继续以 unsupported 结束。
3. 收口 `ResumeUnwind` cleanup contract，覆盖 cleanup state、source slice、origin/resume-state、frame root release 和 ordinary return path 下的 frame 生命周期。
4. 修正 `main(args)` 路由：
   其问题必须通过 outward-empty plain callable routing 解决，而不是再引入 Step argv ABI 或特判 wrapper。
5. 将 plain callable emission 中对残留 `Perform` / `ResumeUnwind` / `Handle` / `Todo` 的拒绝改写为基于 actual outward effect contract 的 verifier，而不是“内部 shape 分类器”。
6. 为 effect-typed callable adapter 与 cleanup/unwind 行为补齐 run-pass、IR 单测和必要的 runtime regression。

必须遵从的约束：

- P4 不得通过再发明一套临时 ABI、临时 wrapper 或 main 特例来绕开 actual outward effect routing。
- P4 不得把内部 effect/control shape 当 ABI source of truth。
- P4 与 P5 共享的 composite payload contract 必须通过中立 transport API 收口，不得在 cleanup/unwind 或 adapter 里各自发明一套 carrier。

阶段输出：

- callable ABI routing 只由 actual outward effect set 决定。
- effect-typed callable value surface 完整可用。
- cleanup/unwind 与 `main(args)` contract 收口。

验证：

1. `cargo test -p scoopc refactor_llvm_source_classification_verifier`
2. `cargo test -p scoopc refactor_llvm_resume_unwind_lowering`
3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
4. `cargo test -p scoopc llvm_tests`
5. 回归时重点关注 `effect_handle_return_from_function_any_boxing.scoop` 与 `main(args)` 相关样本。

完成条件：

- `§3.12`、`§5.1`、`§5.3`、`§5.4` 关闭。
- outward-empty callable 不再误入 effect-step entry。
- effect-typed callable value / `FunPtr` 不再是默认主线缺口。

### P5. 统一 aggregate/composite transport，关闭 enum/array/closure/boxing 残余缺口

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.8、§3.11、§4.1、§4.3、§4.4、§4.5。

目标：

- 把 composite value transport 从“每条路径单独补特殊规则”收口为统一 contract。
- 关闭 enum payload、array composite element、closure env / capture shape 和 aggregate boxing 的残余缺口。
- 让 raw MIR、effect-lowered LLVM、runtime payload transport 共享同一套复合值语义。

必须实现的内容：

1. 为 large integer enum payload、nested enum payload、tuple/struct/non-scalar payload 建立统一 boxed/transport layout，不再假定单 word payload。
2. 为 `Array.get` / `MutableArray.set` 建立完整 composite element metadata 与 transport path，移除退回 `u64` 路径时的 unsupported。
3. 收口 closure env / capture transport contract：
   默认主线接受的 capture shape 都必须有明确 env layout / boxing / store/load 语义；
   其余不属于默认主线的 capture shape 应前移 gate，而不是留给 raw MIR codegen unsupported。
4. 关闭 `§4.1` 的 aggregate boxing residual，把“boxing 已有、transport 未统一”的中间状态收口为单一实现。
5. 收口 pattern runtime type test 的 narrow residual：
   对默认主线接受的 pattern `is Type` surface，runtime test 必须闭合；
   若某类 target 不属于默认主线，必须在 frontend 或 MIR 更早 gate。
6. 确保 P4 使用的 effect payload / unwind payload / cross-boundary composite carrier 直接复用本阶段 transport contract。

必须遵从的约束：

- P5 不得继续保留“enum 一套、array 一套、effect payload 一套、closure env 一套”的孤立 transport 规则。
- P5 的完成标准不是“更多 unsupported 变少了”，而是默认主线接受的 composite surface 都已经有统一 contract 或明确 gate。
- P5 不得把 current `Partial` 长期保留为“以后再统一”。

阶段输出：

- 统一的 composite transport / boxing / layout contract。
- enum payload、array composite element、closure env / capture shape 都可由统一路径处理。
- `§4.1` 从 residual partial 归零。

验证：

1. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
2. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
3. `cargo test -p scoopc llvm_tests`
4. `cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport`

完成条件：

- `§3.8`、`§3.11`、`§4.1`、`§4.3`、`§4.4`、`§4.5` 关闭或被明确前移 gate。
- 默认主线 composite surface 不再依赖 scattered unsupported 分支。

### P6. 同步 frontend gates、收尾 partial surface、重写 gap 分类到最终状态

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.5、§7.1、§7.2、§7.3、§7.5、§7.6。

目标：

- 在 live gap 基本补齐后，把剩余非默认主线 surface 全部前移成清晰 gate 或正式实现。
- 清除“文档已关、代码仍半开”“前端已挡、后端仍半支持”“inventory 仍把旧编号当 blocker”的状态不一致。
- 把 `PIPELINE_GAPS.md` 和 executable inventory 重写成最终分类。

必须实现的内容：

1. 对 `FrontendReject` / re-scoped surface 明确最终策略：
   `or-pattern binder`、function type runtime cast、struct mutable field 若本轮不打开，则前端 gate、diagnostic、MIR verifier 和 backend capability 必须完全一致；`use-site effect row type arg` 若已具 production 能力，则必须改写为正式支持并删除过时的 FrontendReject 叙述。
2. 收尾 `§3.5` 与 `§7.6` 的 partial residual：
   要么把默认主线已接受的子集补齐到 production；
   要么缩小接受面并前移为明确 gate；
   不允许继续保持 `Partial`。
3. 复写 `PIPELINE_GAPS.md` 中所有本轮 touched 条目：
   live gap 改成 `Closed/Re-scoped`；
   删除后的 `LegacyOnly` 条目改成 `Historical` 或明确的“legacy producer removed”说明；
   closed items 不再被描述成默认 blocker。
4. 清理 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`、`mir/placeholder_inventory.rs` 等 executable inventories 中与旧主线、closed blocker、stale owner 绑定的条目。
5. 刷新相关 fixtures、IR 单测和 dump 断言，去掉对旧 blocker 文本和旧 fallback 行为的依赖。

必须遵从的约束：

- P6 不得把尚未真正实现的 live gap 通过改文档状态“关闭”掉。
- P6 不得因为想让 `Partial` 清零，就草率地扩大 `FrontendReject` 面。
- P6 结束后，active code 中不应再出现 `LegacyOnly`、旧 fallback reason 或“文档说关闭但 executable inventory 还当 blocker”的矛盾状态。
- P6 结束后，`FrontendReject` 的用户可见文案必须统一表达“输入非法/当前语言不接受该输入”，不得继续使用“后端尚未支持”式描述。

阶段输出：

- 最终版 gap 分类。
- 与默认主线能力一致的 frontend gates。
- 不再混杂旧主线残留的 executable inventory 与 fixtures。

验证：

1. `cargo test -p scoopc codegen_gap_inventory`
2. `cargo test -p scoopc llvm_tests`
3. `cargo run -p scoop -- test`
4. 对活跃代码树执行以下搜索，确认命中只剩文档或 archive：
   `LegacyOnly`
   `UnsupportedMainBody`
   `assign lhs lowering pending`
   `call callee lowering pending`
   `resume lowering requires canonical callee shape`

完成条件：

- 默认主线相关 `Partial = 0`。
- active code 中 `LegacyOnly = 0`。
- `PIPELINE_GAPS.md`、inventories、fixtures 的状态叙述与真实能力一致。

### P7. 执行 full regression、grep 审计和阶段退出复核

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §9。

目标：

- 用完整测试矩阵证明默认主线已闭合、旧主线 residual code 已清空。
- 在结束前做一次“文档、代码、inventory、fixture 四边一致性”审计。

必须实现的内容：

1. 运行完整验证矩阵并修复所有回归。
2. 对 active tree 做 legacy residual grep 审计，确认旧 reason string、不再允许的 inventory bucket、旧 fallback helper、旧 blocker 文本都已消失。
3. 复核 `UnsupportedMainBody` / unsupported bucket：
   它们只能表达真正 impossible-state 或当前仍显式 gate 的 surface，不得再映射到“旧主线遗留分支还在”。
4. 复核 `§3.7`、`§6.3` 等已 closed 项的 regression coverage，确保不因本轮删除旧代码而回退。
5. 将本计划完成状态回写到 `PIPELINE_GAPS.md` 和相关 archive / 任务文档。
6. 复核所有用户可见失败路径，确认它们都能归入“明确诊断的非法输入”而不是“模糊 unsupported”。

必须遵从的约束：

- P7 不通过，整轮工作不算完成。
- 若 full regression 暴露出新的 legacy residual reachability，不得把问题降级为“历史 gap 已知”；必须回到对应 owner phase 修复或删除。

阶段输出：

- 完整的回归记录。
- 一份 clean active tree 审计结果。
- 与仓库现实一致的最终 gap 账本。

验证：

1. `cargo test --all`
2. `cargo run -p scoop -- test`
3. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
4. `cargo test -p scoopc llvm_tests`
5. `cargo test -p scoopc codegen_gap_inventory`

完成条件：

- 默认主线 live gap 全部关闭。
- 旧主线 residual code 不再存在于 active tree。
- 文档、inventory、fixture 与实现对同一事实给出一致结论。
- 对合法输入，编译 pipeline 不再以 unsupported / fallback / assertion 作为用户可见结果；对非法输入，错误信息明确稳定。

## 6. 验证矩阵

- 基线：`cargo test --all`
- fixture 主线：`cargo run -p scoop -- test`
- GC/runtime 主线：`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
- MIR / placeholder / handoff 相关：`cargo test -p scoopc refactor_hir_placeholder_inventory`、`cargo test -p scoopc refactor_mir_placeholder_inventory`、`cargo test -p scoopc codegen_gap_inventory`
- effect-refactor / cleanup / cross-thread 相关：`cargo test -p scoopc refactor_llvm_source_classification_verifier`、`cargo test -p scoopc refactor_llvm_resume_unwind_lowering`、`cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport`
- IR / runtime surface 回归：`cargo test -p scoopc llvm_tests`
- legacy 清理审计：对 active tree 搜索 `LegacyOnly` 和 P0 固定的 legacy reason 词表，确认命中只剩 `PIPELINE_GAPS.md`、`docs/archive/**` 或其他文档路径
- 用户可见失败路径审计：搜索并复核 `UnsupportedMainBody`、`Unsupported*`、`todo!`、`panic!`、`unreachable!` 所在生产路径，确认它们只充当 internal bug sentinel 或测试辅助，不承担合法/非法输入的业务分流

## 7. 完成标准

本轮完成时，必须能同时陈述以下结论全部成立：

1. `PIPELINE_GAPS.md` 中默认主线相关 `Open` 与 `Partial` 已归零。
2. active code、active tests、active inventories 中不再存在 `LegacyOnly` bucket、旧主线 residual producer、旧 fallback reason string。
3. production MIR、materialized MIR、raw MIR route、effect-refactor ABI、aggregate/composite transport 都只依赖新主线 contract，不再借 legacy fallback 或 late unsupported 补洞。
4. `FrontendReject` surface 与 backend 真实能力完全一致；未开放的 surface 都在更早阶段明示拒绝。
5. `§3.7`、`§6.3` 等已关闭项保持关闭，没有因旧代码删除而回归。
6. `cargo test --all`、`cargo run -p scoop -- test`、GC/runtime 矩阵和关键 LLVM tests 全部通过。
7. 对合法有效输入，编译器要么产出正确结果，要么暴露为可定位的编译器 bug；对非法输入，编译器返回明确错误，不再以“尚未支持的特性”等模糊说法充当结果。
