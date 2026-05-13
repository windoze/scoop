# TODO（新主线收口与旧主线清理）

> 生成时间：2026-05-14  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 差距基线：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md)  
> 格式参考：`docs/archive/plans/TODO-stable-id.md`、`docs/archive/plans/TODO-pipeline-gaps-mir.md`、`docs/archive/plans/TODO-pipeline-gaps-codegen.md`  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 当前状态：当前默认/refactor 主线仍存在 pre-MIR/MIR handoff、raw MIR route、effect ABI/routing、aggregate/composite transport 四类 live gap；同时旧主线 residual producer、`LegacyOnly` inventory bucket、legacy reason string、以及一部分“文档已关闭但代码仍残留”的 fallback 仍在 active tree 中。  
> 最终 contract：整个编译 pipeline 对用户只允许两类结果：合法有效输入产生正确输出；非法输入返回明确、稳定、可定位到源码的错误信息。除此之外的一切行为都视为编译器 bug，而不是“尚未支持的特性”。

## 任务索引

| ID | 阶段 | 标题 |
| --- | --- | --- |
| `P0-T01` | P0 | 建立 active inventory / legacy reason 审计基线 |
| `P0-T02` | P0 | 固定“非法输入 vs 编译器 bug”的用户可见失败策略 |
| `P1-T01` | P1 | 删除 `mir/lower.rs` 中 assign/call/ctor/intrinsic legacy producer |
| `P1-T02` | P1 | 删除 resume/dispatch legacy producer，并清空 active `LegacyOnly` 依赖 |
| `P2-T01` | P2 | 关闭 `comptime_*` 与 top-level `val` 的 pre-MIR/MIR gap |
| `P2-T02` | P2 | 收紧 production MIR verifier，拒绝 `unterminated` 与 `Return { value: None }` 漏洞 |
| `P2-T03` | P2 | 收紧 materialization/root/no-param handoff，并把 `§2.3` 降为 pure impossible-state guard |
| `P3-T01` | P3 | 收口 raw MIR terminator/call-kind/`PerformResult` route policy |
| `P3-T02` | P3 | 收口 ctor/default-arg typed contract，删除 backend 补参/猜测 |
| `P3-T03` | P3 | 收口 `StoreMember` continuation route 与 raw function-ref normalization regression |
| `P4-T01` | P4 | 让 actual outward effect set 唯一决定 callable ABI，并补齐 effect-typed callable adapter |
| `P4-T02` | P4 | 收口 cleanup/unwind contract 与 `main(args)` plain routing |
| `P5-T01` | P5 | 统一 composite transport contract，关闭 enum/array boxing residual |
| `P5-T02` | P5 | 收口 closure env/capture transport 与 pattern `is Type` residual |
| `P6-T01` | P6 | 收尾 `§3.5` / `§7.6` partial surface，统一 runtime cast 与 GC pin/handle policy |
| `P6-T02` | P6 | 同步 `FrontendReject` surface：or-pattern binder / function-type cast / use-site effect row / struct mutable field |
| `P6-T03` | P6 | 重写 `PIPELINE_GAPS.md`、active inventory 与 fixtures 到最终状态 |
| `P7-T01` | P7 | 执行 full regression 与 legacy residual grep 审计 |
| `P7-T02` | P7 | 审计所有用户可见失败路径并完成最终回写 |

## 全局约束

- [`PLAN.md`](./PLAN.md) 是本轮唯一计划基线。当前文件只负责把 `PLAN.md` 的 P0-P7 拆成严格顺序执行的任务；`docs/archive/plans/**` 只作格式与历史参考。
- [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) 是唯一 gap 账本。若任务执行中改变某个 gap 的状态、owner、route 或“是否属于默认主线”的判断，必须先回写该文件，再继续实现。
- 整个编译 pipeline 的最终用户可见 contract 只有两类结果：
  - 合法有效输入产生正确输出。
  - 非法输入返回明确、稳定、可定位到源码的错误信息。
- 除上述两类结果外，其余一切行为都视为编译器 bug，包括但不限于：`UnsupportedMainBody`、其他 `Unsupported*`、`Todo(...)`、panic、assertion、静默 default-value fallback、late unsupported bucket、误编译。
- `FrontendReject` 只表示“该输入在当前语言 contract 下非法，必须以前端明确诊断拒绝”；它不是“后端还没实现所以先报 unsupported”的同义词。
- `Historical` / `Closed/Re-scoped` 只允许继续存在于文档或 regression 审计中；它们不得继续以 active production blocker 的形式出现在 executable inventory 里。
- `LegacyOnly` 的最终目标是 active code 中归零。历史映射可以保留在文档中，但 active inventory、active tests、active guards、active reason string 里不允许继续保留 `LegacyOnly` bucket。
- 严禁把旧主线 residual branch 变成“理论上不可达的 dormant if 分支”继续留在 active tree。
- 原则上不允许新增新的 placeholder reason、late unsupported 文案或临时 fallback；如果确有必要，必须先更新 inventory、owner 和本文件。
- raw MIR、effect-lowered LLVM、runtime C 只能消费显式 handoff contract、facts、layout metadata 与 target/session config；不得回 HIR/旧 MIR fallback 语义补洞。
- 每个任务完成后，必须在对应条目的“完成记录”中回写：
  - 改动范围
  - 核心决策
  - 验证结果
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合

## P0：冻结基线与失败策略

### [DONE] P0-T01：建立 active inventory / legacy reason 审计基线

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P0
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §0、§8、§9
- 目标：
  - 在删除旧主线代码和关闭 live gap 之前，先把 active inventory、legacy residual reason 和 repo-scan 审计入口固定下来。
  - 让后续任务不需要反复重新搜索 `LegacyOnly`、legacy reason 字符串、closed-but-still-blocking inventory entry。
- 当前实现入口：
  - `crates/scoopc/src/mir/placeholder_inventory.rs`
  - `crates/scoopc/src/hir/lower/placeholder_inventory.rs`
  - `crates/scoopc/src/llvm/codegen_gap_inventory.rs`
  - `PIPELINE_GAPS.md`
  - 当前已知必须纳入审计的 legacy reason：
    - `assign lhs missing local`
    - `assign lhs lowering pending`
    - `call callee lowering pending`
    - `ctor call lowering pending`
    - `sizeOf intrinsic requires value or type arg`
    - `nameOf intrinsic requires type arg`
    - `resume lowering requires canonical callee shape`
    - `dispatch callee lowering pending`
- 必须实现的内容：
  1. 在现有 inventory 测试旁新增或扩展一个 cross-inventory 审计入口。
     - 推荐位置：`crates/scoopc/src/pipeline_gap_audit.rs`，或在 `mir/placeholder_inventory.rs` / `llvm/codegen_gap_inventory.rs` 下增加等价 `#[cfg(test)]` helper。
     - 它至少要扫描 `crates/scoopc/src`、`crates/scoop/src`、`tests/fixtures`。
  2. 固化以下审计输出：
     - active tree 中 `LegacyOnly` 的命中列表
     - 上述八个 legacy reason 的命中列表
     - `codegen_gap_inventory.rs` 中已 `Closed/Re-scoped` 但仍被当 production blocker 的 gap id 列表
  3. 为 inventory 建立一份明确的分类规则说明，并落到测试注释或 helper 常量里：
     - live contract
     - downstream impossible-state guard
     - frontend reject
     - historical-only mapping
  4. 明确记录本轮退出条件：
     - `Open = 0`
     - 默认主线相关 `Partial = 0`
     - active code 中 `LegacyOnly = 0`
  5. 对 `crates/scoopc/src/llvm/codegen_gap_inventory.rs` 的当前基线做一次审计记录。
     - 当前已知仍列在 inventory 中、但文档已关闭或重定 scope 的编号至少包括：`§3.4`、`§3.7`、`§4.1`、`§4.2`、`§5.2`、`§5.5`、`§5.6`、`§5.7`、`§6.1`、`§6.2`、`§6.3`、`§6.4`、`§6.5`。
- 必须遵从的约束：
  - 本任务只建立审计基线，不提前删除 gap entry 或改行为。
  - 不允许把“inventory 里有命中”直接当成 bug；本任务要做的是先把命中与分类关系固化下来。
  - 审计输出必须可被后续任务直接复用，而不是一次性的手工 grep 备注。
- 验证：
  1. `cargo test -p scoopc refactor_hir_placeholder_inventory`
  2. `cargo test -p scoopc refactor_mir_placeholder_inventory`
  3. `cargo test -p scoopc codegen_gap_inventory`
  4. 推荐新增：`cargo test -p scoopc pipeline_gap_audit -- --nocapture`
  5. 额外执行搜索并把命中摘要写入完成记录：
     - `rg 'LegacyOnly' crates/scoopc/src crates/scoop/src tests/fixtures`
     - `rg 'assign lhs missing local|assign lhs lowering pending|call callee lowering pending|ctor call lowering pending|sizeOf intrinsic requires value or type arg|nameOf intrinsic requires type arg|resume lowering requires canonical callee shape|dispatch callee lowering pending' crates/scoopc/src crates/scoop/src tests/fixtures`
- 完成条件：
  - 后续每个任务都能直接引用同一份 active inventory / legacy reason 审计基线。
  - 不再需要先靠人工 grep 才能知道旧主线 residual 还剩哪些入口。
- 依赖：无
- 完成记录：
  - 改动范围：
    - 新增 `crates/scoopc/src/pipeline_gap_audit.rs`，固定 cross-inventory 审计入口、搜索根、分类规则、退出条件、active-tree `LegacyOnly` 基线、八个 legacy reason 基线，以及 codegen inventory scope-drift / closed-re-scoped blocker 基线。
    - 更新 `crates/scoopc/src/lib.rs`，通过 `#[cfg(test)] mod pipeline_gap_audit;` 接入新的审计测试模块。
  - 核心决策：
    - 审计入口放在独立测试模块中，而不是散落到现有 inventory 测试里，后续任务只需更新一处基线即可复用同一套 repo-scan 结果。
    - 分类规则统一冻结为四类：`live contract`、`downstream impossible-state guard`、`frontend reject`、`historical-only mapping`；退出条件固定为 `Open = 0`、默认主线相关 `Partial = 0`、active code 中 `LegacyOnly = 0`。
    - 为避免 `rg 'LegacyOnly' ...` 和 legacy reason 搜索被新审计模块自身污染，词表和预期命中行在测试里采用运行时拼接，不把目标字符串原样留在 `pipeline_gap_audit.rs` 源文件中。
    - `codegen_gap_inventory.rs` 的当前基线分成两层冻结：scope-drift baseline 为 `§3.4`、`§3.7`、`§4.1`、`§4.2`、`§5.2`、`§5.5`、`§5.6`、`§5.7`、`§6.1`、`§6.2`、`§6.3`、`§6.4`、`§6.5`；其中已 `Closed/Re-scoped` 且仍是 `production_blocker` 的子集为 `§3.4`、`§3.7`、`§4.2`、`§5.2`、`§5.5`、`§5.6`、`§5.7`、`§6.1`、`§6.2`、`§6.3`、`§6.4`。
  - 验证结果：
    - `cargo test -p scoopc refactor_hir_placeholder_inventory`
    - `cargo test -p scoopc refactor_mir_placeholder_inventory`
    - `cargo test -p scoopc codegen_gap_inventory`
    - `cargo test -p scoopc pipeline_gap_audit -- --nocapture`
    - `cargo clippy --all-targets -- -D warnings`
    - `rg 'LegacyOnly' crates/scoopc/src crates/scoop/src tests/fixtures`：16 个命中，仅在 `crates/scoopc/src/hir/lower/placeholder_inventory.rs` 与 `crates/scoopc/src/mir/placeholder_inventory.rs`。
    - `rg 'assign lhs missing local|assign lhs lowering pending|call callee lowering pending|ctor call lowering pending|sizeOf intrinsic requires value or type arg|nameOf intrinsic requires type arg|resume lowering requires canonical callee shape|dispatch callee lowering pending' crates/scoopc/src crates/scoop/src tests/fixtures`：35 个命中，分布于 `mir/lower.rs`、`mir/materialize.rs`、`mir/mod.rs`、`mir/placeholder_inventory.rs`、`pipeline/hir_preflight.rs`、`pipeline/hir_stage.rs`、`pipeline/mir_stage.rs`。
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P0 第 1-5 项：active inventory / legacy reason / codegen scope-drift 的审计边界已冻结为可执行测试。
    - 本任务只建立审计基线，不改变 `PIPELINE_GAPS.md` 中任何状态、owner 或 route；后续任务可直接引用审计模块输出更新对应条目。

### [DONE] P0-T02：固定“非法输入 vs 编译器 bug”的用户可见失败策略

- 参考：
  - [`PLAN.md`](./PLAN.md) §0、§5 / P0、P7
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §0、§9
- 目标：
  - 在开始收口实现前，先把“什么是明确诊断的非法输入”“什么是 production path 上绝对不应再向用户暴露的编译器 bug”固定下来。
  - 让后续 agent 不会把 `UnsupportedMainBody`、panic 或 late fallback 继续当成正常错误形态。
- 当前实现入口：
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/mir/materialize.rs`
  - `crates/scoopc/src/pipeline/mir_stage.rs`
  - `crates/scoopc/src/pipeline/hir_stage.rs`
  - `crates/scoopc/src/typecheck/lower.rs`
  - `crates/scoopc/src/typecheck/when_pat.rs`
  - `crates/scoopc/src/typecheck/structs.rs`
- 必须实现的内容：
  1. 建立一份 production-path failure audit 清单，覆盖至少以下关键词：
     - `UnsupportedMainBody`
     - `Unsupported`
     - `todo!`
     - `panic!`
     - `unreachable!`
  2. 对每个命中归入三类之一，并把分类规则写进测试注释或 helper 常量：
     - 非法输入的显式前端诊断
     - internal bug sentinel / impossible-state guard
     - 仍需后续任务消除的 stale user-visible failure
  3. 对 `FrontendReject` 的语义做一次集中整理。
     - 当前相关入口至少有：
       - `crates/scoopc/src/typecheck/when_pat.rs` 的 or-pattern binder reject
       - `crates/scoopc/src/pipeline/mir_stage.rs` 的 function-type runtime cast reject
       - `crates/scoopc/src/typecheck/lower.rs` 的 use-site effect row type arg reject
       - `crates/scoopc/src/typecheck/structs.rs` 的 struct mutable field reject
     - 明确它们的诊断文案不允许继续表达成“后端尚未支持”。
  4. 推荐新增 source inventory / 审计测试，防止新的 production-path `Unsupported*` 或 `panic!` 在没有分类说明的情况下落地。
     - 推荐命名：`pipeline_user_visible_failure_policy_*`
- 必须遵从的约束：
  - 本任务不删除实现，只冻结“失败类型”的边界和允许列表。
  - 不能把所有 `panic!` / `unreachable!` 一刀切改成普通诊断；本任务要先区分 internal bug sentinel 和 stale user-visible failure。
  - 不得把 parser/typecheck 已接受的输入继续归类为“非法输入”。
- 验证：
  1. 推荐新增：`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  2. `cargo test -p scoopc codegen_gap_inventory`
  3. `rg 'UnsupportedMainBody|Unsupported[A-Za-z_]+|todo!|panic!|unreachable!' crates/scoopc/src`
- 完成条件：
  - 后续任务已经有统一的“非法输入 vs 编译器 bug”判断标准。
  - 用户可见失败路径的审计入口已经固定，不再需要临时口头约定。
- 依赖：`P0-T01`
- 完成记录：
  - 改动范围：
    - 新增 `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`，固定 production-path failure audit：分类规则、审计文件范围、`UnsupportedMainBody` 当前基线、stale `Unsupported*` 标记、production `todo! = 0` 基线，以及 `panic!` / `unreachable!` internal sentinel 基线。
    - 更新 `crates/scoopc/src/lib.rs`，通过 `#[cfg(test)] mod pipeline_user_visible_failure_policy;` 接入新的审计测试模块。
    - 更新 `crates/scoopc/src/typecheck/expr/error.rs` 与 `crates/scoopc/src/typecheck/when_pat.rs`，为 `when` 的 or-pattern binder reject 提升独立诊断代码 `scoop::typecheck::when_or_pattern_binder_not_allowed`，不再复用 generic `UnsupportedExpr`。
    - 更新 `crates/scoopc/src/typecheck/structs.rs`，将 `struct` 可变字段拒绝文案收紧为“必须是 `val`，不允许 `var`”。
    - 同步更新 `tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_{is_error,sharing_is_error}.scoop` 与 `tests/fixtures/typecheck/struct_{primary_ctor_var_is_error,field_must_be_val_is_error}.scoop` 的预期文案/错误码。
  - 核心决策：
    - 将 failure policy 审计做成独立测试模块，并只统计 production slice（遇到 `#[cfg(test)]` 即截断），避免把单元测试里的 `panic!` / `UnsupportedMainBody` 噪音混入生产路径基线。
    - `FrontendReject` 统一冻结为“前端显式诊断拒绝非法输入”，并集中锁定四个关键 surface：or-pattern binder、function-type runtime cast、use-site effect row type arg、struct mutable field。
    - 当前仍暴露在生产路径上的 `UnsupportedMainBody` 明确归类为 `stale user-visible failure`，后续任务必须逐步将其替换为真实实现、verifier failure 或更早的 frontend reject，而不是继续当成可接受的用户结果。
    - `panic!` / `unreachable!` 当前只允许以 internal bug sentinel 身份保留；新的审计基线锁定了现有 20 个命中，后续若新增必须先说明分类。
  - 验证结果：
    - `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
    - `cargo test -p scoopc codegen_gap_inventory`
    - `cargo test -p scoopc refactor_mir_value_primitives_reject_unsupported_function_type_cast_before_mir`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_sharing_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/struct_primary_ctor_var_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/struct_field_must_be_val_is_error.scoop`
    - `cargo clippy --all-targets -- -D warnings`
    - `rg 'UnsupportedMainBody|Unsupported[A-Za-z_]+|todo!|panic!|unreachable!' crates/scoopc/src`：repo-wide grep 仍能看到大量现存 `UnsupportedMainBody` / `Unsupported*` 命中；新的审计基线将当前任务入口冻结为 815 个 `UnsupportedMainBody`、4 个 stale typecheck `Unsupported*` 标记、0 个 production `todo!`、20 个 internal bug sentinel。
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P0 第 6 项，以及 `PIPELINE_GAPS.md` §0、§7.1、§7.2、§7.3、§7.5：`非法输入 vs 编译器 bug` 的可执行边界已被固定到测试中，后续任务可以直接复用同一套分类与审计出口。
    - 本任务只冻结失败类型边界与允许列表，不改变 `PLAN.md` 阶段顺序，也不修改 `PIPELINE_GAPS.md` 的状态、owner 或 route。

## P1：删除旧主线 residual producer 与 `LegacyOnly` 依赖

### [TODO] P1-T01：删除 `mir/lower.rs` 中 assign/call/ctor/intrinsic legacy producer

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P1
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.6、§1.7、§6.3
- 目标：
  - 删除 `mir/lower.rs` 中仍会主动产出 legacy Todo reason 的 assign/call/ctor/intrinsic fallback。
  - 强制 MIR lowering 只消费 typed place contract、typed call-site contract、selected ctor contract、typed reflection intrinsic contract。
- 当前实现入口：
  - `crates/scoopc/src/mir/lower.rs::lower_assign_stmt`
    - 当前会生成 `StatementKind::Todo("assign lhs missing local")`
    - 当前会生成 `StatementKind::Todo("assign lhs lowering pending")`
  - `crates/scoopc/src/mir/lower.rs` 的普通 call lowering 路径
    - 当前会生成 `Rvalue::Todo("call callee lowering pending")`
    - 当前会生成 `Rvalue::Todo("ctor call lowering pending")`
  - `crates/scoopc/src/mir/lower.rs::lower_reflection_intrinsic_call_expr`
    - 当前会生成 `Rvalue::Todo("sizeOf intrinsic requires value or type arg")`
    - 当前会生成 `Rvalue::Todo("nameOf intrinsic requires type arg")`
  - 对应的 current consumer / tests：
    - `crates/scoopc/src/pipeline/hir_preflight.rs`
    - `crates/scoopc/src/pipeline/mir_stage.rs::refactor_mir_place_contract_lowers_assignment_places`
    - `crates/scoopc/src/pipeline/mir_stage.rs::refactor_mir_call_contract_lowers_typed_call_sites`
    - `tests/fixtures/mir_refactor/assignment_places.scoop`
    - `tests/fixtures/mir_refactor/call_contracts.scoop`
- 必须实现的内容：
  1. 删除 `lower_assign_stmt` 中旧的 non-typed fallback 分支。
     - `lower_assign_stmt_with_place_contract(...)` 应成为唯一 production 路径。
     - 若 place contract 缺失，应在更早阶段以明确诊断或 strict verifier 失败结束，而不是再落 `Todo(...)`。
  2. 删除普通 call lowering 中的 `call callee lowering pending` / `ctor call lowering pending` producer。
     - direct / closure / fun value / ctor 都必须来自 typed call-site contract。
     - 若 contract 漏洞暴露，必须改成清晰 diagnostic 或 verifier failure。
  3. 删除 reflection intrinsic fallback。
     - `sizeOf` / `nameOf` 必须只依赖 typed value/type-arg contract。
     - 不允许再通过“有值就拿值、没有就临时猜 type arg”的 fallback 生产 Todo。
  4. 更新对应的 forbidden-list / smoke test。
     - `refactor_mir_place_contract_lowers_assignment_places`
     - `refactor_mir_call_contract_lowers_typed_call_sites`
     - 若这些测试当前只验证“不泄漏 legacy reason”，补一个更强的正向断言，验证 place contract / direct call / ctor contract / metadata primitive 仍然存在。
  5. 如果删除 legacy producer 后暴露出 typed contract 发布缺口，不在本任务中回滚 fallback；应把缺口作为真正 bug 暴露给 P2/P3。
- 必须遵从的约束：
  - 不得把这些 legacy reason 换个措辞继续作为新的 `Todo(...)`。
  - 不得保留 dormant `if !uses_refactor_typed_contracts()` 分支。
  - 不得把 `sizeOf` / `nameOf` 的 fallback 继续藏在 helper 函数里。
- 验证：
  1. `cargo test -p scoopc refactor_mir_place_contract`
  2. `cargo test -p scoopc refactor_mir_call_contract`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/assignment_places.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/call_contracts.scoop`
  5. `rg 'assign lhs missing local|assign lhs lowering pending|call callee lowering pending|ctor call lowering pending|sizeOf intrinsic requires value or type arg|nameOf intrinsic requires type arg' crates/scoopc/src`
- 完成条件：
  - 上述六个 legacy producer 不再存在于 `mir/lower.rs` 的 active production path。
  - 相关失败全部转化为 typed contract 缺失的明确错误，而不是 Todo/unsupported。
- 依赖：`P0-T02`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

### [TODO] P1-T02：删除 resume/dispatch legacy producer，并清空 active `LegacyOnly` 依赖

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P1
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.8、§1.9
- 目标：
  - 删除 legacy resume / dispatch producer 与所有依赖这些 reason 的 active guard、inventory 和测试白名单。
  - 把 active code 中 `LegacyOnly` bucket 清零。
- 当前实现入口：
  - `crates/scoopc/src/mir/lower.rs::lower_resume_call_expr`
    - 当前会生成 `Rvalue::Todo("resume lowering requires canonical callee shape")`
  - `crates/scoopc/src/mir/lower.rs` 的 dispatch lowering
    - 当前通过 `callee_fqn.rsplit_once('.')` 尝试恢复 owner/member
    - 失败时会生成 `Rvalue::Todo("dispatch callee lowering pending")`
  - `crates/scoopc/src/mir/placeholder_inventory.rs`
    - 当前仍有 `LegacyOnly` entries 和 `PlaceholderDisposition::LegacyOnly`
  - `crates/scoopc/src/hir/lower/placeholder_inventory.rs`
    - 当前虽然无 active legacy entry，但仍显式保留 `LegacyOnly` disposition 及相关断言
  - `crates/scoopc/src/pipeline/hir_preflight.rs`
    - 当前仍把上述 legacy reason 记入 preflight 禁止词表
  - `crates/scoopc/src/mir/mod.rs`
    - `is_forbidden_refactor_effect_todo(...)` 和单测仍直接使用 legacy reason
  - `crates/scoopc/src/mir/materialize.rs`
    - 现有单测仍用 legacy reason 构造负例
  - `tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`
  - `tests/fixtures/mir_refactor/continuation_resume_unit_sugar.scoop`
- 必须实现的内容：
  1. 删除 `lower_resume_call_expr` 中对 canonical callee shape 的 legacy fallback，强制 resume lowering 只走 typed resume contract。
  2. 删除 dispatch lowering 中通过 `rsplit_once('.')` 猜 owner/member 的残余路径，强制消费 structured dispatch contract。
  3. 删除 active code 中的 `LegacyOnly` disposition 和所有 related assert/helper。
     - `mir/placeholder_inventory.rs`
     - `hir/lower/placeholder_inventory.rs`
  4. 清理与 legacy reason 绑定的 active tests / guard / whitelist。
     - `pipeline/hir_preflight.rs`
     - `pipeline/mir_stage.rs`
     - `mir/mod.rs`
     - `mir/materialize.rs`
  5. 将仍然需要“no Todo verifier”覆盖的单测改成 synthetic / contract-neutral test reason，而不是继续绑死已删除的 legacy string。
     - 例如：`refactor_mir_no_todo_rejects_statement_todo`、materializer 负例等，不应再以 `assign lhs lowering pending` 作为唯一测试输入。
- 必须遵从的约束：
  - 不得把 `LegacyOnly` 枚举仅仅改名后保留在 active inventory。
  - 不得留下“legacy producer 已删，但 active tests 还在拿 legacy reason 当 canonical failure 文案”的半清理状态。
  - 删除这些入口后若暴露出 `UnsupportedMainBody` / panic / assertion，只能说明真实 bug 被揭露；不得回退恢复 fallback。
- 验证：
  1. `cargo test -p scoopc refactor_mir_placeholder_inventory`
  2. `cargo test -p scoopc refactor_mir_call_contract`
  3. `cargo test -p scoopc refactor_materialized_mir`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/continuation_resume_unit_sugar.scoop`
  6. `rg 'resume lowering requires canonical callee shape|dispatch callee lowering pending|LegacyOnly' crates/scoopc/src crates/scoop/src tests/fixtures`
- 完成条件：
  - active code、active tests、active inventories 中的 `LegacyOnly` 依赖归零。
  - resume / dispatch 只再依赖 typed contract，不再依赖 legacy shape recovery。
- 依赖：`P1-T01`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

## P2：收紧 pre-MIR / MIR handoff

### [TODO] P2-T01：关闭 `comptime_*` 与 top-level `val` 的 pre-MIR/MIR gap

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.1、§1.4
- 目标：
  - 在 runtime MIR 之前彻底消除 `comptime block/if/for` placeholder。
  - 让 top-level `val` 拥有真实 MIR root / initializer model，而不是 `Item::Todo`。
- 当前实现入口：
  - `crates/scoopc/src/mir/placeholder_inventory.rs`
    - `comptime_block`
    - `comptime_if`
    - `comptime_for`
    - `top-level val`
  - `crates/scoopc/src/pipeline/hir_preflight.rs`
  - `crates/scoopc/src/pipeline/hir_stage.rs`
  - `crates/scoopc/src/pipeline/mir_stage.rs`
  - `crates/scoopc/src/mir/mod.rs`
  - 现有 fixture：
    - `tests/fixtures/mir_refactor/comptime_splice_class_with_update.scoop`
    - `tests/fixtures/mir_refactor/top_level_roots.scoop`
    - `tests/fixtures/typecheck/top_level_val_with_type_ok.scoop`
    - `tests/fixtures/typecheck/top_level_val_pattern_inferred_same_file_ok.scoop`
- 必须实现的内容：
  1. 确保 `comptime block/if/for` 在进入 runtime MIR 之前被展开或明确诊断。
     - 本任务不重新打开 splice field / class literal / with-update 的已关闭 gap。
     - 若相关逻辑回归导致这些 surface 重新生成 Todo，本任务必须顺手修正，而不是留给后续阶段。
  2. 为 top-level `val` 建立正式的 MIR item/root 表达。
     - 目标至少包括：declaration identity、initializer root、callable root 查询、与 object init / extern global 并列的 root index。
  3. 让 `tests/fixtures/mir_refactor/top_level_roots.scoop` 与 `pipeline/mir_stage.rs` 的相关断言不再依赖 `Item::Todo` 消失，而是直接断言新的 root model 存在。
  4. 若 top-level `val` 与 package/object init ordering 有耦合，需把 ordering contract 一并写清，不得再靠 HIR side table 或 ad hoc reachability 恢复。
- 必须遵从的约束：
  - 不得把 `top-level val` 从 inventory 里删掉却不落地真正的 MIR root。
  - `comptime_*` 失败必须在更早阶段给出明确错误，不得再转化为 MIR placeholder。
- 验证：
  1. `cargo test -p scoopc refactor_hir_placeholder_inventory`
  2. `cargo test -p scoopc refactor_mir_placeholder_inventory`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/comptime_splice_class_with_update.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/top_level_roots.scoop`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/top_level_val_with_type_ok.scoop`
- 完成条件：
  - `comptime_block` / `comptime_if` / `comptime_for` 不再进入 runtime MIR。
  - `top-level val` 不再以 `Item::Todo` 表示，且有可查询的 canonical MIR root。
- 依赖：`P1-T02`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

### [TODO] P2-T02：收紧 production MIR verifier，拒绝 `unterminated` 与 `Return { value: None }` 漏洞

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §2.1、§2.4
- 目标：
  - 把 `unterminated` builder sentinel 和 non-`Unit` `Return { value: None }` 前移成 production verifier failure。
  - 杜绝 raw MIR codegen 为 non-`Unit` 函数偷偷合成默认返回值。
- 当前实现入口：
  - `crates/scoopc/src/mir/placeholder_inventory.rs`
    - `unterminated`
  - `crates/scoopc/src/mir/mod.rs`
    - `MirFile::validate_refactor_production(...)`
    - 当前相关单测：`refactor_mir_no_todo_*`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - 当前仍有 `Return { value: None }` fallback/default-value path
  - 现有 fixture：
    - `tests/fixtures/mir_refactor/while_break_continue.scoop`
    - `tests/fixtures/mir_refactor/handle_perform.scoop`
- 必须实现的内容：
  1. 确保 production MIR verifier 把 `unterminated` 视为 hard failure，而不是“builder 后续可能会修好”。
  2. 删除 raw MIR codegen 中对 non-`Unit` `Return { value: None }` 的默认值发射路径。
  3. 统一 `Return { value: None }` contract：
     - `Unit` 返回允许 `None`
     - 非 `Unit` 返回必须在 MIR 中显式带值，否则 verifier 直接拒绝
  4. 为该规则补齐 negative tests。
     - 推荐新增：`refactor_mir_no_return_none_*`
     - 若现有 `refactor_mir_no_todo_*` 已覆盖，可直接扩展
  5. 若 break/continue/finally 等控制流路径会间接制造 `unterminated` 或 `Return None`，必须一起修正 CFG 生成，而不是只在 verifier 层补白名单。
- 必须遵从的约束：
  - 不得把 `Return None` 问题继续留给 codegen stage 兜底。
  - 不得通过“默认值恰好等于 0/null”式 silent fallback 维持旧行为。
- 验证：
  1. `cargo test -p scoopc refactor_mir_no_todo`
  2. `cargo test -p scoopc refactor_mir_placeholder_inventory`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/while_break_continue.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/handle_perform.scoop`
- 完成条件：
  - `unterminated` 不再能越过 production MIR verifier。
  - non-`Unit` `Return { value: None }` 不再进入 codegen，且不再由 codegen 合成默认返回值。
- 依赖：`P2-T01`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

### [TODO] P2-T03：收紧 materialization/root/no-param handoff，并把 `§2.3` 降为 pure impossible-state guard

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §2.3、§2.5、§2.7
- 目标：
  - 让 materialized MIR 成为真正的 canonical codegen input：无 Todo、无 missing root、无 concrete path `TypeKind::Param`。
  - 把 `§2.3` 从“仍算 open gap”收缩成“下游 impossible-state guard”。
- 当前实现入口：
  - `crates/scoopc/src/mir/materialize.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
  - 现有 fixture：
    - `tests/fixtures/mir_refactor/generic_materialization.scoop`
    - `tests/fixtures/run-pass/generic_fun_recursion.scoop`
  - 现有测试入口：
    - `cargo test -p scoopc refactor_materialized_mir`
    - `cargo test -p scoopc codegen_gap_inventory`
- 必须实现的内容：
  1. 固化 materializer 对 missing generic template / missing MIR root 的 source-level hard error。
  2. 彻底消除 concrete path 上 `TypeKind::Param` 到达 codegen 的可能。
     - direct call materialization
     - frame slot / return / aggregate transport
     - effect-lowered callable carrier / payload transport
  3. 复核 codegen side 对 `TypeKind::Param` 的剩余 guard。
     - guard 可以保留，但语义必须改成 impossible-state / compiler bug sentinel。
     - 不能再作为默认主线“尚未支持 generic concrete path”的正常结果。
  4. 复核 `codegen_gap_inventory.rs` 中 `§2.3` 对应的说明，使之反映“production MIR contract guard”，而不是 live feature gap。
  5. 补齐 materialized root index / instance key 测试，确保 generic callable、top-level root、object/member root 都能由 materialized snapshot 完整查询。
- 必须遵从的约束：
  - 不得通过 fallback FQN、默认 type arg 或 late codegen guess 来“解决” missing root / unresolved param。
  - 不得把 `TypeKind::Param` guard 直接删除后失去最终 bug sentinel。
- 验证：
  1. `cargo test -p scoopc refactor_materialized_mir`
  2. `cargo test -p scoopc codegen_gap_inventory`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/generic_materialization.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/generic_fun_recursion.scoop`
  5. `rg 'TypeKind::Param|MaterializedTodo|MissingMirRootForTemplate|MissingGenericTemplate' crates/scoopc/src`
- 完成条件：
  - successful materialized MIR 不再含 Todo / missing root / concrete path `TypeKind::Param`。
  - `§2.3` 的下游 Todo guard 仅剩 impossible-state 审计语义。
- 依赖：`P2-T02`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

## P3：收口 raw MIR route 与 call/member contract

### [TODO] P3-T01：收口 raw MIR terminator/call-kind/`PerformResult` route policy

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.1、§3.2、§3.3、§3.6
- 目标：
  - 明确 raw MIR emitter 只接受 raw-safe 输入。
  - 让 effect/control terminator、unsupported call kind 与 `PerformResult` 都在 route gate 或完整 lowering 中收口，不再晚到 body emission 才炸。
- 当前实现入口：
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - 当前 raw terminator gate
    - 当前 `Perform` cleanup / `resume_target` unsupported 路径
    - 当前 `Rvalue::PerformResult` default-value path
    - 当前 `Virtual` / `Interface` / `Resume` call kind unsupported 路径
  - `crates/scoopc/src/llvm/codegen_gap_inventory.rs`
    - `§3.1`、`§3.2`、`§3.3`、`§3.6`
  - 现有测试入口：
    - `cargo test -p scoopc refactor_llvm_backend_gate`
    - 若缺少 raw-route 定向测试，应在 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 或等价位置补新增：
      - `refactor_llvm_raw_route_gate_*`
      - `raw_mir_effect_control_route_*`
  - build fixture：
    - `tests/fixtures/build/emit_llvm_basic.scoop`
    - `tests/fixtures/build/effect_refactor_direct_handle_resume_emit_llvm.scoop`
- 必须实现的内容：
  1. 对 raw MIR route 建立明确 policy：
     - `Handle`
     - `ResumeUnwind`
     - raw `Perform`
     - raw `PerformResult`
     - `CallKind::Virtual`
     - `CallKind::Interface`
     - `CallKind::Resume`
     以上要么完整 lower，要么在 route verifier 前拒绝并改走 late-lowered/published boundary。
  2. 删除 `PerformResult` 的默认值路径。
  3. `Perform` 若仍不能在 raw path lower cleanup/unwind，必须在 route gate 之前 fail-fast，不得再把缺 `resume_target` / cleanup contract 的 body 交给 raw emitter。
  4. 对 unsupported call kind 的错误文案和 inventory 说明改成“route bug / missing handoff contract”，而不是模糊 unsupported。
- 必须遵从的约束：
  - 不得在 raw MIR route 内部实现第二套 handler/resume/cleanup 语义来兜底。
  - 不得继续保留 `PerformResult` default-value 这类 silent miscompile。
  - route gate 失败必须是明确的 compiler bug/contract violation，不是用户层“尚未支持 raw MIR 形状”。
- 验证：
  1. `cargo test -p scoopc refactor_llvm_backend_gate`
  2. 若本任务新增 raw-route 定向测试：`cargo test -p scoopc refactor_llvm_raw_route_gate -- --nocapture`
  3. 若本任务新增 effect-control route 定向测试：`cargo test -p scoopc raw_mir_effect_control_route -- --nocapture`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_direct_handle_resume_emit_llvm.scoop`
- 完成条件：
  - `§3.1`、`§3.2`、`§3.3`、`§3.6` 不再在 production path 晚期触发 unsupported/default-value fallback。
- 依赖：`P2-T03`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

### [TODO] P3-T02：收口 ctor/default-arg typed contract，删除 backend 补参/猜测

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.9、§3.10
- 目标：
  - 让 ctor selected binding、named/default arg canonicalization 都在 upstream contract 中闭合。
  - 让 backend 不再承担补齐参数、容忍 arity drift、猜 selected ctor 的职责。
- 当前实现入口：
  - `crates/scoopc/src/pipeline/mir_stage.rs::refactor_mir_call_contract_lowers_typed_call_sites`
  - `crates/scoopc/src/mir/lower.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - 现有 fixture：
    - `tests/fixtures/mir_refactor/call_contracts.scoop`
    - `tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop`
    - `tests/fixtures/run-pass/default_param_call_site_fill_basic.scoop`
    - `tests/fixtures/typecheck/default_param_named_args_mid_omit_ok.scoop`
    - `tests/fixtures/typecheck/default_param_named_args_skip_non_default_is_error.scoop`
- 必须实现的内容：
  1. 确保 ctor lowering 使用完整的 selected ctor + ordered bound args contract。
     - 不再允许 backend 通过 callee shape 或参数数量去猜 ctor。
  2. 完成默认参数 canonicalization。
     - named arg / default arg 的最终顺序、缺省值选择、receiver 位置都必须在 MIR contract 中固定。
  3. 删除 backend 中任何“arity 不匹配时再补一轮”的逻辑或错误文案。
  4. 若 ctor / default arg contract 仍依赖 call-site binding side table，必须让 side table publication 成为 authoritative handoff，并补测试锁定。
  5. 对当前 run-pass 支持的 ctor named/default/delegation 保持行为回归无漂移。
- 必须遵从的约束：
  - 不得把当前 run-pass 已支持的 surface 通过扩大 `FrontendReject` 面来“解决”。
  - 不得继续让 backend 做 order repair、missing arg fill 或 selected ctor 猜测。
- 验证：
  1. `cargo test -p scoopc refactor_mir_call_contract`
  2. `cargo test -p scoopc llvm_tests`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/call_contracts.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/default_param_call_site_fill_basic.scoop`
- 完成条件：
  - `§3.9`、`§3.10` 不再依赖 backend 猜测/补齐。
  - ctor/default arg drift 若再次出现，会在 MIR contract 层明确暴露。
- 依赖：`P3-T01`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

### [TODO] P3-T03：收口 `StoreMember` continuation route 与 raw function-ref normalization regression

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.7、§3.13
- 目标：
  - 对 `StoreMember` 的 continuation route 歧义建立 upstream resolve/reject 规则。
  - 保持 top-level callable value / `FunPtr` 已关闭的 regression 不回退。
- 当前实现入口：
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - `StoreMember` continuation route lowering
    - `TopLevelRef` / function reference normalization 路径
  - 现有测试：
    - `crates/scoopc/src/llvm/codegen/mir_body.rs::refactor_mir_member_access_codegen_rejects_unresolved_metadata`
    - `crates/scoopc/src/llvm/codegen/mir_body.rs::refactor_mir_store_member_codegen_rejects_ambiguous_continuation_route`
  - 现有 fixture：
    - `tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`
    - `tests/fixtures/mir_refactor/assignment_places.scoop`
- 必须实现的内容：
  1. 确立 `StoreMember` continuation route 的 authoritative upstream contract。
     - `Ambiguous` 不能继续进入 LLVM。
     - upstream 必须 resolve 或 reject。
  2. 调整 MIR lowering / verifier / codegen gate，使 ambiguous route 在进入 emitter 前失败。
  3. 保持 `§3.7` 的 regression audit：
     - raw MIR 不得重新直接发射未规范化的普通函数引用
     - `FunPtr` / top-level callable value 继续通过 run-pass 与 IR 级回归锁定
  4. 若本任务需要触及 function-ref normalization helper，必须保证它不重新引入 legacy producer 或 string-based callee recovery。
- 必须遵从的约束：
  - 不得在 LLVM emitter 现场“猜一个最可能的 continuation route”。
  - 不得把 `§3.7` 重新打开成 live blocker；该项只允许做 regression 审计。
- 验证：
  1. `cargo test -p scoopc refactor_mir_member_access_codegen`
  2. `cargo test -p scoopc refactor_mir_store_member_codegen`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/assignment_places.scoop`
- 完成条件：
  - `§3.13` 关闭。
  - `§3.7` 继续保持 closed/re-scoped，且相关 regression 未回退。
- 依赖：`P3-T02`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

## P4：收口 effect-refactor ABI、adapter 与 unwind/main 路由

### [TODO] P4-T01：让 actual outward effect set 唯一决定 callable ABI，并补齐 effect-typed callable adapter

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.12、§5.1、§5.4
- 目标：
  - 让 actual outward effect set 成为 callable ABI 的唯一分类依据。
  - 为 plain closure / function-value / `FunPtr` 在 effect-typed surface 上补齐 adapter 或 published boundary。
- 当前实现入口：
  - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - 现有测试入口：
    - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs::refactor_llvm_no_outward_plain_abi_layout_has_no_step_shell`
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs::refactor_llvm_function_abi_entry_shells_use_refactor_direct_entry`
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs::refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry`
  - 现有 fixture：
    - `tests/fixtures/run-pass/effect_typed_plain_adapter_aggregate_return_basic.scoop`
    - `tests/fixtures/run-pass/receiver_function_value_call_basic.scoop`
    - `tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_higher_order_when_direct.scoop`
- 必须实现的内容：
  1. 审计 callable ABI routing。
     - outward-empty callable 必须发布 plain ABI。
     - actual outward 非空或 adapter surface 才发布 EffectStep / adapter。
  2. 补齐 effect-typed callable adapter。
     - closure
     - function-value
     - `FunPtr`
  3. 删除任何按内部 effect/control shape 直接分类 ABI 的残余逻辑。
  4. 为 plain/effect adapter publication 补回归，确保 direct entry / wrapper / dynamic shell 的角色关系稳定。
  5. 若需要新增 helper，helper 的职责必须是 routing 或 adapter publication，不能重新引入“旧主线 plain wrapper”一类独立逻辑。
- 必须遵从的约束：
  - 不得通过再造一套临时 ABI 或“特殊 wrapper 族”绕开 actual outward effect routing。
  - 不得把 outward-empty callable 继续误路由成 step entry。
- 验证：
  1. `cargo test -p scoopc refactor_llvm_function_abi_entry_shells_use_refactor_direct_entry`
  2. `cargo test -p scoopc refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_typed_plain_adapter_aggregate_return_basic.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/receiver_function_value_call_basic.scoop`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_higher_order_when_direct.scoop`
- 完成条件：
  - `§3.12`、`§5.1`、`§5.4` 关闭。
  - plain callable / function-value / `FunPtr` 在默认主线接受的 effect-typed surface 上不再返回 unsupported。
- 依赖：`P3-T03`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

### [TODO] P4-T02：收口 cleanup/unwind contract 与 `main(args)` plain routing

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §5.3、§5.4
- 目标：
  - 完整定义 `ResumeUnwind` / cleanup state / frame release contract。
  - 让 `main(args)` 通过 outward-empty plain routing 正确落到 plain entry，而不是继续误入 effect-step wrapper。
- 当前实现入口：
  - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
    - current unwind / cleanup verifier and lowering
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
    - `refactor_llvm_main_wrapper_routes_unhandled_outward_to_exit_code`
    - `refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry`
  - 现有 fixture：
    - `tests/fixtures/run-pass/effect_raise_cleanup_gc_basic.scoop`
    - `tests/fixtures/run-pass/effect_handle_return_from_function_finally.scoop`
    - `tests/fixtures/run-pass/effect_handle_return_from_function_basic.scoop`
    - `tests/fixtures/run-pass/std_process_args_exit_basic.scoop`
- 必须实现的内容：
  1. 明确 cleanup/unwind contract 中至少以下信息的 authoritative 来源：
     - cleanup state
     - origin/resume-state
     - source slice
     - frame root release 时机
  2. 对 ordinary return path、tail return path、cleanup path、runtime-error path 的 frame 生命周期建立一致规则。
  3. 修正 `main(args)` 路由。
     - 问题必须通过 outward-empty plain routing 解决。
     - 不允许再发明 Step argv ABI 或 main 特判 wrapper 家族。
  4. 复核主线 `main` wrapper 行为：
     - outward-empty plain 走 plain entry
     - 只有真实 outward effect body 才走 effect-step / exit-code path
- 必须遵从的约束：
  - 不得继续让 cleanup/unwind contract 只支持“空 placeholder cleanup”这一窄路径。
  - 不得用 `main` 特例掩盖 callable ABI routing 问题。
- 验证：
  1. `cargo test -p scoopc refactor_llvm_resume_unwind_lowering`
  2. `cargo test -p scoopc refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_raise_cleanup_gc_basic.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_return_from_function_finally.scoop`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/std_process_args_exit_basic.scoop`
- 完成条件：
  - `§5.3`、`§5.4` 关闭。
  - cleanup/unwind 与 `main(args)` 的用户可见行为稳定且不再依赖 workaround。
- 依赖：`P4-T01`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

## P5：统一 aggregate / composite transport

### [TODO] P5-T01：统一 composite transport contract，关闭 enum/array boxing residual

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §4.1、§4.3、§4.4、§4.5
- 目标：
  - 建立统一 composite transport / boxing / layout contract。
  - 收口 enum payload、array composite element、aggregate boxing 的残余缺口。
- 当前实现入口：
  - `crates/scoopc/src/llvm/codegen/composite_transport.rs`
  - `crates/scoopc/src/llvm/codegen/enum_lowering.rs`
  - `crates/scoopc/src/llvm/codegen/control_flow.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - 现有测试入口：
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs::refactor_llvm_composite_transport_contract_emits_layout_descriptor_globals`
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs::refactor_llvm_value_boxing_transport`
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs::refactor_llvm_enum_payload_transport`
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs::refactor_llvm_array_composite_transport`
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs::refactor_llvm_cross_thread_resume_payload_transport`
  - 现有 fixture：
    - `tests/fixtures/mir_refactor/aggregate_transport.scoop`
    - `tests/fixtures/run-pass/enum_payload_boxing_any_basic.scoop`
    - `tests/fixtures/run-pass/enum_oversized_variant_boxing_suppressed.scoop`
    - `tests/fixtures/run-pass/option_nested_custom_enum_payload_basic.scoop`
    - `tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop`
    - `tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_composite.scoop`
- 必须实现的内容：
  1. 抽出或收口 single-source composite transport contract。
     - inline vs boxed
     - erased carrier
     - value boxing
     - array element transport
     - effect payload reuse
  2. 关闭 large integer enum payload 的单-word 假设。
  3. 关闭 nested enum/tuple/struct payload unsupported 分叉。
  4. 关闭 array composite element metadata 缺失时退回 `u64` 路径的行为。
  5. 确保 cross-thread resume payload 已有的 composite transport regression 继续复用同一套 contract，而不是重新开分叉。
- 必须遵从的约束：
  - 不得再保留“enum 一套、array 一套、effect payload 一套、boxing 一套”的长期并行 contract。
  - 不得把当前已通过的 runtime_gc composite payload regression 重新打坏。
- 验证：
  1. `cargo test -p scoopc refactor_llvm_composite_transport_contract_emits_layout_descriptor_globals`
  2. `cargo test -p scoopc refactor_llvm_value_boxing_transport`
  3. `cargo test -p scoopc refactor_llvm_enum_payload_transport`
  4. `cargo test -p scoopc refactor_llvm_array_composite_transport`
  5. `cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport`
  6. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_payload_boxing_any_basic.scoop`
  7. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop`
  8. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_composite.scoop`
- 完成条件：
  - `§4.1`、`§4.3`、`§4.4`、`§4.5` 关闭。
  - composite transport 成为单一 authoritative contract。
- 依赖：`P4-T02`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

### [TODO] P5-T02：收口 closure env/capture transport 与 pattern `is Type` residual

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.8、§3.11
- 目标：
  - 让默认主线接受的 closure capture shape 都有完整 env transport contract。
  - 收口 pattern runtime type test 的残余窄面。
- 当前实现入口：
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - closure env / capture lowering
    - pattern `is Type` lowering
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
    - `refactor_llvm_closure_env_transport`
    - `refactor_llvm_runtime_type_primitives`
  - 现有 fixture：
    - `tests/fixtures/mir_refactor/pattern_is_type.scoop`
    - `tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop`
    - `tests/fixtures/run-pass/parameterized_supertype_interface_dispatch.scoop`
    - `tests/fixtures/runtime_gc/gc_trace_closure_capture_string_basic.scoop`
    - `tests/fixtures/runtime_gc/gc_move_enum_maybe_ref_closure_capture_basic.scoop`
- 必须实现的内容：
  1. 为当前默认主线接受的 closure capture shape 明确 env layout / boxing / load/store contract。
  2. 若某些 capture shape 不属于默认主线，必须把 reject/gate 前移，而不是继续在 raw MIR codegen unsupported。
  3. 收口 pattern `is Type` residual。
     - 对默认主线当前接受的类/接口/value-type runtime test 给出完整实现。
     - 若 function-type / effectful function-type pattern 仍不属于默认主线，必须保持更早 reject，不得晚到 backend unsupported。
  4. 复核 closure env transport 与 P5-T01 的 composite transport 是否共享同一套 descriptor/trace contract。
- 必须遵从的约束：
  - 不得把 closure env/capture 问题重新拆成独立 transport 规则。
  - 不得把 pattern `is Type` 的 residual 继续长期保留为 `Partial`。
- 验证：
  1. `cargo test -p scoopc refactor_llvm_closure_env_transport`
  2. `cargo test -p scoopc refactor_llvm_runtime_type_primitives`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/pattern_is_type.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_trace_closure_capture_string_basic.scoop`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_move_enum_maybe_ref_closure_capture_basic.scoop`
- 完成条件：
  - `§3.8`、`§3.11` 关闭或被前移为明确 gate。
  - closure env / capture 不再留给 raw MIR codegen unsupported。
- 依赖：`P5-T01`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

## P6：同步 frontend gate、收尾 partial surface、重写账本

### [TODO] P6-T01：收尾 `§3.5` / `§7.6` partial surface，统一 runtime cast 与 GC pin/handle policy

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P6
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.5、§7.6
- 目标：
  - 让 runtime type/value primitive 的 residual narrow surface 与 GC pin/handle 支持面都明确落在“正式支持”或“更早 reject”两类中。
  - 清除 `Partial` 的长期挂起状态。
- 当前实现入口：
  - `crates/scoopc/src/pipeline/mir_stage.rs`
    - runtime cast/typecheck gate
    - GC pin/handle MIR contract
  - `crates/scoopc/src/typecheck/expr/error.rs`
    - GC pin/handle diagnostics
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - runtime type primitive lowering
    - GC pin/handle lowering
  - 现有 fixture：
    - `tests/fixtures/mir_refactor/runtime_typecheck_cast.scoop`
    - `tests/fixtures/run-pass/gc_pin_unpin_basic.scoop`
    - `tests/fixtures/runtime_gc/gc_pin_unpin_move_stress_matrix.scoop`
    - `tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop`
    - `tests/fixtures/runtime_gc/gc_handle_token_roundtrip_callback_basic.scoop`
    - `tests/fixtures/runtime_gc/gc_handle_stale_callback_token_is_error.scoop`
- 必须实现的内容：
  1. 明确 runtime cast/typecheck 的最终支持面。
     - 默认主线接受的 surface 必须有完整 MIR + LLVM 路径。
     - 其余 surface 必须前移 reject，不再保留半支持状态。
  2. 明确 GC pin/unpin / handleNew/Get/Drop / callback token 的最终支持面。
     - 默认主线接受的 surface 必须补齐 typed contract、lowering 与 runtime regression。
     - 其余 surface 必须以前端明确 reject。
  3. 统一相关诊断文案，使其表达“输入非法/contract 不满足”，而不是“后端尚未支持”。
  4. 更新 `PIPELINE_GAPS.md` / inventory 所需的最终分类信息，但正式文档回写放在 `P6-T03`。
- 必须遵从的约束：
  - 不得为了清掉 `Partial` 而简单缩小默认主线能力。
  - 不得把 runtime_gc 已覆盖的 GC handle/pin 行为重新降回 frontend reject。
- 验证：
  1. `cargo test -p scoopc refactor_mir_value_primitives`
  2. `cargo test -p scoopc refactor_llvm_runtime_type_primitives`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/runtime_typecheck_cast.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_pin_unpin_basic.scoop`
  5. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_pin_unpin_move_stress_matrix.scoop`
  6. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop`
- 完成条件：
  - `§3.5`、`§7.6` 不再保持 `Partial`。
  - runtime cast 与 GC pin/handle 的默认主线能力边界明确且可测试。
- 依赖：`P5-T02`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

### [TODO] P6-T02：同步 `FrontendReject` surface：or-pattern binder / function-type cast / use-site effect row / struct mutable field

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P6
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §7.1、§7.2、§7.3、§7.5
- 目标：
  - 把剩余 `FrontendReject` surface 的语义、diagnostic、MIR gate、backend capability 一次性对齐。
  - 保证这些 reject 都被表达成“当前语言 contract 下非法输入”，而不是“后端还没实现”。
- 当前实现入口：
  - `crates/scoopc/src/typecheck/when_pat.rs`
  - `crates/scoopc/src/pipeline/mir_stage.rs`
  - `crates/scoopc/src/typecheck/lower.rs`
  - `crates/scoopc/src/typecheck/structs.rs`
  - 现有 fixture：
    - `tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_sharing_is_error.scoop`
    - `tests/fixtures/typecheck/fn_type_cast_effectful_asq_is_error.scoop`
    - `tests/fixtures/typecheck/fn_type_cast_effectful_as_is_error.scoop`
    - `tests/fixtures/parse/type_args_eff_use_site_order_fail.scoop`
    - `tests/fixtures/typecheck/struct_property_setter_not_allowed_is_error.scoop`
    - `tests/fixtures/typecheck/class_var_property_reassign_ok.scoop`
- 必须实现的内容：
  1. 统一这四类 surface 的 reject policy 文案。
     - 必须明确是“当前语言 contract 下非法输入”或“当前阶段不接受该源码形状”。
  2. 复核 parser/typecheck/MIR gate 是否一致。
     - parser 接受但 typecheck reject 的 surface 要有稳定解释。
     - 不能出现 typecheck 通过、MIR 再以 unsupported 拒绝的分裂状态。
  3. 复核 backend / verifier 路径是否还有相应半支持逻辑。
     - 若有，必须删掉或降为 internal bug sentinel。
  4. 如其中某个 surface 其实已经具备 production 级能力，则应把它从 `FrontendReject` 升级为正式支持，并补相应 fixtures；不能因为文档写了 reject 就强行保持关闭。
  5. `use-site effect row type arg` 当前只有 parse 级样本；若仓库里缺少明确的 typecheck/frontend reject fixture，本任务必须补一个最小负例，再把它纳入常驻验证。
- 必须遵从的约束：
  - 不得把“当前 parse/typecheck 没拦住”的编译器 bug 直接改写成更宽泛的 `FrontendReject`。
  - 不得让 `FrontendReject` 成为吸纳未知 backend bug 的垃圾桶。
- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_sharing_is_error.scoop`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_effectful_asq_is_error.scoop`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_effectful_as_is_error.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/parse/type_args_eff_use_site_order_fail.scoop`
  5. 若本任务新增了 use-site effect row type arg 的 typecheck/frontend reject fixture，也必须补跑该 fixture
  6. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/struct_property_setter_not_allowed_is_error.scoop`
- 完成条件：
  - `§7.1`、`§7.2`、`§7.3`、`§7.5` 的 reject 语义与实际实现完全一致。
  - 不再出现“前端写 reject、后端仍半支持”或“前端未拒绝、后端再报 unsupported”的分裂状态。
- 依赖：`P6-T01`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

### [TODO] P6-T03：重写 `PIPELINE_GAPS.md`、active inventory 与 fixtures 到最终状态

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P6
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) 全文，尤其 §0、§8、§9
- 目标：
  - 在实现收口后，重写账本与 inventory，使其与真实能力一致。
  - 把“文档已关、inventory 仍当 blocker”“legacy 已删、active tests 仍绑 legacy string”这种状态不一致一次性清掉。
- 当前实现入口：
  - `PIPELINE_GAPS.md`
  - `crates/scoopc/src/mir/placeholder_inventory.rs`
  - `crates/scoopc/src/hir/lower/placeholder_inventory.rs`
  - `crates/scoopc/src/llvm/codegen_gap_inventory.rs`
  - `crates/scoopc/src/llvm/tests.rs`
  - `crates/scoopc/src/pipeline/mir_stage.rs`
  - `tests/fixtures/**`
- 必须实现的内容：
  1. 将本轮已关闭的 live gap 逐项改写为最终状态。
     - 对 live gap：改成 `Closed/Re-scoped`
     - 对已删除 legacy producer：改成 `Historical` 或明确注明“legacy producer removed”
  2. 从 active inventory 移除已经不再作为默认 blocker 的 closed ids。
     - 尤其是 `codegen_gap_inventory.rs` 中当前仍列着的 closed/re-scoped 编号。
  3. 刷新 active tests / fixtures / IR 断言，去掉对 legacy reason、stale blocker 文案、旧 unsupported trigger 的依赖。
  4. 为仍然保留的 impossible-state guard 更新 trigger 文案，使其表达“contract violation / compiler bug sentinel”，而不是旧 gap 名称。
  5. 对 `§3.7`、`§6.3` 等已 closed 项保留 regression coverage，但不再让它们留在 active blocker inventory 中。
- 必须遵从的约束：
  - 不能用改文档状态代替真实修复。
  - 不能让 archive 文档、active TODO、active inventory 互相矛盾。
- 验证：
  1. `cargo test -p scoopc codegen_gap_inventory`
  2. `cargo test -p scoopc refactor_mir_placeholder_inventory`
  3. `cargo test -p scoopc llvm_tests`
  4. `cargo run -p scoop -- test`
  5. `rg 'LegacyOnly|assign lhs lowering pending|call callee lowering pending|resume lowering requires canonical callee shape|UnsupportedMainBody' crates/scoopc/src crates/scoop/src tests/fixtures`
- 完成条件：
  - `PIPELINE_GAPS.md`、active inventory、active fixtures 对同一事实给出一致结论。
  - active tree 不再出现 `LegacyOnly` 和旧 fallback reason。
- 依赖：`P6-T02`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

## P7：执行 full regression 与最终审计

### [TODO] P7-T01：执行 full regression 与 legacy residual grep 审计

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P7
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §9
- 目标：
  - 用完整测试矩阵证明默认主线已闭合。
  - 用 active-tree grep 审计证明旧主线 residual code 已清空。
- 当前实现入口：
  - 全仓库 active code
  - 特别关注：
    - `crates/scoopc/src/mir/lower.rs`
    - `crates/scoopc/src/mir/placeholder_inventory.rs`
    - `crates/scoopc/src/llvm/codegen_gap_inventory.rs`
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
  - legacy residual reason 词表沿用 `P0-T01`
- 必须实现的内容：
  1. 运行 full test matrix。
  2. 对 active tree 执行 legacy residual grep 审计。
     - `LegacyOnly`
     - 八个 legacy reason
     - `UnsupportedMainBody`
  3. 对发现的剩余命中逐一分类：
     - 文档/archive 保留
     - 合法的 internal bug sentinel
     - 仍需清理的 active residual
  4. 若 full regression 暴露出新 blocker，必须回到对应任务修复，不允许在此任务里简单记录“known issue”后结束。
- 必须遵从的约束：
  - P7 不能只是“命令跑完了”；必须附命中摘要和分类结果。
  - 若 grep 仍在 active tree 命中 legacy residual，不得宣告完成。
- 验证：
  1. `cargo test --all`
  2. `cargo run -p scoop -- test`
  3. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
  4. `cargo test -p scoopc llvm_tests`
  5. `cargo test -p scoopc codegen_gap_inventory`
  6. `rg 'LegacyOnly|assign lhs missing local|assign lhs lowering pending|call callee lowering pending|ctor call lowering pending|sizeOf intrinsic requires value or type arg|nameOf intrinsic requires type arg|resume lowering requires canonical callee shape|dispatch callee lowering pending|UnsupportedMainBody' crates/scoopc/src crates/scoop/src tests/fixtures`
- 完成条件：
  - 全量回归通过。
  - active tree 中不再留有旧主线 residual 命中。
- 依赖：`P6-T03`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：

### [TODO] P7-T02：审计所有用户可见失败路径并完成最终回写

- 参考：
  - [`PLAN.md`](./PLAN.md) §0、§5 / P7
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §0、§9
- 目标：
  - 最终确认用户可见结果已经只剩“正确输出”或“明确错误”。
  - 把 remaining `Unsupported*` / `panic!` / `todo!` / `unreachable!` 全部归位到 internal bug sentinel 或删除。
- 当前实现入口：
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/mir/materialize.rs`
  - `crates/scoopc/src/pipeline/hir_stage.rs`
  - `crates/scoopc/src/pipeline/mir_stage.rs`
  - `crates/scoopc/src/typecheck/**/*.rs`
- 必须实现的内容：
  1. 复核所有 production-path `Unsupported*` / `panic!` / `todo!` / `unreachable!` 命中。
  2. 对剩余命中逐个写清：
     - internal bug sentinel
     - test-only helper
     - should-have-been-frontend-diagnostic bug（若还有则继续修）
  3. 确认所有 `FrontendReject` surface 的最终用户文案都表达“非法输入/当前语言 contract 不接受”，而不是“尚未支持”。
  4. 将最终结果回写到：
     - `PLAN.md`（如需补完成状态）
     - `PIPELINE_GAPS.md`
     - 本文件各任务的完成记录
  5. 在最终总结中明确写出以下结论是否成立：
     - 合法输入 -> 正确输出
     - 非法输入 -> 明确错误
     - 其它一律视为编译器 bug
- 必须遵从的约束：
  - 不得用“这些 sentinel 本来就不会触发”替代真实分类说明。
  - 不得留下模糊的 `UnsupportedMainBody` 用户可见路径。
- 验证：
  1. `cargo test --all`
  2. `cargo run -p scoop -- test`
  3. `rg 'UnsupportedMainBody|Unsupported[A-Za-z_]+|todo!|panic!|unreachable!' crates/scoopc/src`
  4. `cargo test -p scoopc llvm_tests`
- 完成条件：
  - 用户可见失败路径审计闭合。
  - 本轮最终 contract 已能被明确陈述且与代码/测试一致。
- 依赖：`P7-T01`
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
