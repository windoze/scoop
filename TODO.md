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
| `P4-T03` | P4 | 隔离 array literal synthetic helper call-site identity，修复 enum ctor contract 污染 |
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

### [DONE] P1-T01：删除 `mir/lower.rs` 中 assign/call/ctor/intrinsic legacy producer

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
    - 更新 `crates/scoopc/src/mir/lower.rs`，删除 assignment / ordinary call / ctor / reflection intrinsic 的 legacy `Todo` producer；`lower_assign_stmt(...)` 现在只走 typed place contract，`lower_call_expr(...)` 现在优先消费 typed call-site contract，并在 contract 缺失时显式暴露 impossible-state panic，而不再生成六个 legacy reason。
    - 更新 `crates/scoopc/src/mir/materialize.rs` 与 `crates/scoopc/src/mir/lower.rs::lower_for_dump(...)`，让 dump/materialize 路径构造 `MirLoweringFacts` 时同步导入 typed call-site / assign-place contract，避免删除 legacy branch 后这些调试/测试入口失去 contract 来源。
    - 更新 `crates/scoopc/src/pipeline/hir_stage.rs`，补 local callable value 的 `FunValue` contract 发布、顶层 `val` 多文件 source-path 解析，以及缺失 typed call-site contract 的 HIR-stage hard error。
    - 更新 `crates/scoopc/src/hir/lower/expr.rs`，为 array-builder / vararg-builder 合成 helper calls 分配可区分的 call span，修复多个 compiler-generated intrinsic 调用共用同一 `CallSite(span)` 导致 typed contract 互相覆盖的问题。
    - 更新 `crates/scoopc/src/pipeline/mir_stage.rs` forbidden-list 断言，并同步 `PIPELINE_GAPS.md` §1.6、§1.7、§6.3 的状态与结论。
  - 核心决策：
    - 不把 dump/materialize 全量切到 `MirSiteContractSource::RefactorTyped`；本任务只把 typed call/assign contract 注入这些入口，保留 `P1-T02` 仍需处理的 resume/dispatch legacy 路径，避免超前改动阶段边界。
    - typed call-site contract 缺失现在在 typed HIR stage 直接失败；唯一允许无显式 call contract 的保留形状是 unresolved enum/`Option` variant ctor/value path，而不是 class ctor / callable value / reflection intrinsic。
    - 对 compiler-generated array-builder/vararg-builder calls 采用“修正 call-site identity”而不是在 MIR lowering 新增 FQN 猜测 fallback：同一合成 block 内的 `new/push/build` helper 现在使用可区分的 span，从根源消除 typed contract 覆盖。
  - 验证结果：
    - `cargo test -p scoopc refactor_hir_call_contracts_record_callable_provenance`
    - `cargo test -p scoopc refactor_hir_class_literal_and_intrinsic_contracts`
    - `cargo test -p scoopc refactor_hir_preflight_checks_completeness_fixtures_and_mir_smoke`
    - `cargo test -p scoopc refactor_mir_place_contract`
    - `cargo test -p scoopc refactor_mir_call_contract`
    - `cargo test -p scoopc dump_mir_lowers_safe_member_access_option_result_without_ctor_todo`
    - `cargo test -p scoopc dump_mir_publishes_member_write_contract_for_escape_continuation_cell`
    - `cargo test -p scoopc materialize_for_dump_dedups_repeated_instance_requests`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/assignment_places.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/call_contracts.scoop`
    - `cargo clippy --all-targets -- -D warnings`
    - `rg 'assign lhs missing local|assign lhs lowering pending|call callee lowering pending|ctor call lowering pending|sizeOf intrinsic requires value or type arg|nameOf intrinsic requires type arg' crates/scoopc/src`：命中仅剩 `mir/placeholder_inventory.rs`、`pipeline/hir_preflight.rs`、`pipeline/mir_stage.rs` 与 `mir/mod.rs` / `mir/materialize.rs` / `mir/lower.rs` / `pipeline/hir_stage.rs` 的 synthetic verifier or test scaffolding；`mir/lower.rs` active production path 已无上述 producer。
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P1 第 1、2、4、6 项：assign/call/ctor/reflection intrinsic legacy producer 已从 active lowering path 移除，dump/materialize 入口同步接入 typed call/assign contract，相关 smoke/forbidden 断言已更新。
    - `PIPELINE_GAPS.md` 已回写 §1.6、§1.7、§6.3：这些 gap 不再描述 active `mir/lower.rs` producer；剩余 legacy reason 清理明确留给 `P1-T02` 的 inventory / guard / synthetic-test residual 收尾。

### [DONE] P1-T02：删除 resume/dispatch legacy producer，并清空 active `LegacyOnly` 依赖

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
    - 更新 `crates/scoopc/src/mir/lower.rs`，删除 `Continuation.resume` 的 canonical-callee legacy fallback 与 dispatch `rsplit_once('.')` owner/member 猜测路径；resume 现在只消费 typed resume contract，dispatch 只消费 typed call-site/member contract，缺失 contract 时直接暴露 impossible-state panic，不再生成 `resume lowering requires canonical callee shape` / `dispatch callee lowering pending`。
    - 更新 `crates/scoopc/src/mir/placeholder_inventory.rs` 与 `crates/scoopc/src/hir/lower/placeholder_inventory.rs`，移除 `LegacyOnly` disposition，以及 MIR inventory 中全部八个 legacy reason 对应条目。
    - 更新 `crates/scoopc/src/pipeline/hir_preflight.rs`、`crates/scoopc/src/pipeline/mir_stage.rs`、`crates/scoopc/src/pipeline/hir_stage.rs`、`crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/materialize.rs`、`crates/scoopc/src/mir/lower.rs` 测试区，删除对 legacy reason 的 active guard/whitelist/synthetic 绑定，改成 contract-neutral 的 no-Todo 断言或 synthetic reason。
    - 更新 `crates/scoopc/src/pipeline_gap_audit.rs`，将 active-tree `LegacyOnly` 命中与 legacy reason 命中基线都收紧为 0；更新 `PIPELINE_GAPS.md` §1.6-§1.9，关闭 dispatch/resume legacy lowering gap，并同步写明 assign/call residual 已从 active inventory/test/guard 中清理。
  - 核心决策：
    - 不把 `lower_for_dump` / materialize 整体切换为 `MirSiteContractSource::RefactorTyped`；本任务只把 resume surface 补接到已有 typed contract，并让 dispatch 继续直接消费 `TypedCallSiteContract::{Virtual,Interface}`，避免超前改动 perform/handle 等尚不在本任务范围内的 handoff 边界。
    - 对 resume/dispatch 缺失 typed contract 的情形，统一升级为 impossible-state panic，而不是继续靠 legacy `Todo(...)` 或 callee 形状恢复语义补洞。
    - synthetic no-Todo 测试不再绑死任何已删除的 legacy reason，统一改成 contract-neutral synthetic reason 或“输出中不应出现任何 `Todo`”的正交断言，避免后续任务继续把旧 reason 当 canonical failure 文案。
  - 验证结果：
    - `cargo test -p scoopc refactor_hir_placeholder_inventory`
    - `cargo test -p scoopc refactor_mir_placeholder_inventory`
    - `cargo test -p scoopc pipeline_gap_audit`
    - `cargo test -p scoopc refactor_mir_call_contract`
    - `cargo test -p scoopc refactor_materialized_mir`
    - `cargo test -p scoopc materialized_pass_view_non_generic_dispatch_and_resume_roots_are_published`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/continuation_resume_unit_sugar.scoop`
    - `cargo clippy --all-targets -- -D warnings`
    - `rg 'LegacyOnly|assign lhs missing local|assign lhs lowering pending|call callee lowering pending|ctor call lowering pending|sizeOf intrinsic requires value or type arg|nameOf intrinsic requires type arg|resume lowering requires canonical callee shape|dispatch callee lowering pending' crates/scoopc/src crates/scoop/src tests/fixtures`：0 命中。
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P1 第 3 项与第 5 项：resume/dispatch legacy producer 已从 active lowering path 删除，active inventory/active tests/active guards 中的 `LegacyOnly` bucket 和 legacy reason 绑定归零。
    - `PIPELINE_GAPS.md` 已回写 §1.8、§1.9 为 `Closed/Re-scoped`，并同步更新 §1.6、§1.7 的 residual 说明：这些 surface 现在只允许通过 typed contract 成功 lowering，缺失 contract 不再以 `Todo(...)` 形式继续流动。

## P2：收紧 pre-MIR / MIR handoff

### [DONE] P2-T01：关闭 `comptime_*` 与 top-level `val` 的 pre-MIR/MIR gap

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
    - 更新 `crates/scoopc/src/hir/lower/stmt.rs`，移除 `comptime block/if/for` 的 `StmtKind::Todo("comptime_*")` 构造点；runtime `comptime if/for` 若缺少求值计划，改为在 HIR lowering 阶段记录明确 stage error，而不是继续把 placeholder 漏给 MIR。
    - 更新 `crates/scoopc/src/mir/lower.rs`，让 `MirLoweringFacts::from_lowered_hir(...)` 也携带 top-level initializer / extern root contract，并让 MIR lowering 统一发射 `InitializerRoot` / `ExternGlobal`；删除 top-level `val -> Item::Todo { kind: "top-level val" }` 的残余分支。
    - 更新 `crates/scoopc/src/hir/lower/placeholder_inventory.rs` 与 `crates/scoopc/src/mir/placeholder_inventory.rs`，把 `comptime_*` 与 `top-level val` 从 active placeholder inventory 中移除；HIR inventory 收紧为零基线 guard。
    - 更新 `crates/scoopc/src/mir/lower.rs` 测试、`crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/pipeline/mir_stage.rs`，补 `dump_mir_emits_top_level_initializer_and_extern_roots` 覆盖，并把 synthetic no-Todo 负例从真实业务 reason 改成 synthetic item reason。
    - 更新 `PIPELINE_GAPS.md`，将 `§1.1` 与 `§1.4` 回写为 `Closed/Re-scoped`。
  - 核心决策：
    - 对 `comptime_*` 采用“主路径必须展开，缺计划则前移失败”的收口方式，而不是继续在 HIR/MIR 中保留 dormant placeholder；`comptime block` 直接展开其 body，`comptime if/for` 必须消费 runtime comptime plan。
    - top-level `val` 的 canonical MIR 表示统一冻结为 `InitializerRoot` / `ExternGlobal` root model；不再允许任何 MIR lowering 路径把它回退成 `Item::Todo`。为避免只有 typed stage 输出正确、`lower_for_dump` 仍缺 root，本次把同一套 root contract 也接入 `from_lowered_hir(...)` 路径。
    - synthetic no-Todo verifier 继续保留，但不再拿 `top-level val` 当 canonical 失败文案，避免已关闭 gap 的 reason 继续滞留在 active 测试语义中。
  - 验证结果：
    - `cargo test -p scoopc refactor_hir_placeholder_inventory`
    - `cargo test -p scoopc refactor_mir_placeholder_inventory`
    - `cargo test -p scoopc dump_mir_emits_top_level_initializer_and_extern_roots`
    - `cargo test -p scoopc refactor_mir_item_graph_publishes_top_level_roots`
    - `cargo test -p scoopc refactor_mir_comptime_splice_class_literal_and_with_update_preclosure`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/comptime_splice_class_with_update.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/top_level_roots.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/top_level_val_with_type_ok.scoop`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P2 第 1 项：runtime `comptime_*` 已不再以 placeholder 形式进入 MIR，top-level `val` 已拥有可查询的 MIR root/initializer model。
    - `PIPELINE_GAPS.md` 已回写 `§1.1`、`§1.4` 为 `Closed/Re-scoped`：这些 surface 现在要么在 HIR lowering 前展开，要么直接以 canonical root model 进入 MIR，不再作为 live placeholder gap 保留。

### [DONE] P2-T02：收紧 production MIR verifier，拒绝 `unterminated` 与 `Return { value: None }` 漏洞

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
    - 更新 `crates/scoopc/src/mir/mod.rs`，把 `unterminated` builder sentinel 纳入 `validate_refactor_direct_style()` 的 forbidden-todo contract，并补 direct-style 负例测试。
    - 更新 `crates/scoopc/src/mir/materialize.rs`，让 materialized MIR 在 non-`Unit` `Return { value: None }` 时返回 `MaterializedMirValidation(RefactorProductionMissingReturnValue)`，并补对应负例测试。
    - 更新 `crates/scoopc/src/llvm/codegen/mir_body.rs`，删除 raw MIR terminator lowering 对 non-`Unit` 空返回的默认值发射路径；新增 `mir_empty_return_contract_is_lowerable(...)` helper 与 raw-codegen 级单测。
    - 更新 `PIPELINE_GAPS.md`，将 `§2.1` 与 `§2.4` 回写为 `Closed/Re-scoped`。
  - 核心决策：
    - `unterminated` 继续保留为 builder 内部 sentinel，但只允许存在于 lowering 过程内部；一旦出现在 direct-style / production handoff 上，就必须由 verifier 立即拒绝，而不是等后续阶段“补全”。
    - non-`Unit` 空返回的 contract 统一冻结为“上游必须显式提供返回值”；stage output 与 materialized output 共用同一条规则，不再让 raw MIR codegen 用零值/null 值悄悄兜底。
    - raw MIR codegen 仍保留最终 contract guard，但 guard 的语义已收紧为“production MIR 被破坏”，不再承担默认返回值修补职责。
  - 验证结果：
    - `cargo test -p scoopc refactor_mir_no_todo`
    - `cargo test -p scoopc refactor_mir_no_return_none`
    - `cargo test -p scoopc refactor_mir_placeholder_inventory`
    - `cargo test -p scoopc refactor_materialized_mir`
    - `cargo test -p scoopc codegen_gap_inventory`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/while_break_continue.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/handle_perform.scoop`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P2 第 3、4、7 项：`unterminated` 已从“允许通过 direct-style validator 的 builder sentinel”收紧为 strict verifier hard failure；`Return { value: None }` contract 已在 production/materialized handoff 上统一，并从 raw MIR codegen 中移除默认值 fallback。
    - `PIPELINE_GAPS.md` 已回写 `§2.1`、`§2.4` 为 `Closed/Re-scoped`；`§2.3` 仍保留为 downstream impossible-state guard bucket，留待 `P2-T03` 继续收口。

### [DONE] P2-T03：收紧 materialization/root/no-param handoff，并把 `§2.3` 降为 pure impossible-state guard

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
    - 更新 `crates/scoopc/src/typecheck/expr/call.rs`，让 `TypeKind::Param` 的 where-bound member call 不再只返回类型；现在会同步发布 `ResolvedMemberRef::Fun`、`TopLevelFunCallBinding`、`CallArgBinding`、monomorph request 与 required effects，确保 `T: ToString` 这类 generic/sysroot 调用能穿过 typed HIR call-contract handoff。
    - 新增 `crates/scoopc/src/pipeline/hir_stage.rs` 回归测试 `refactor_hir_call_contracts_publish_where_bound_member_dispatch`，锁定 `where T: ToString` 上 `value.toString()` 会发布 `TypedCallSiteContract::Interface`。
    - 更新 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`，将 `PIPELINE_GAPS §2.3` 从 production blocker 收紧为 upstream impossible-state guard，并新增定向单测固定该语义。
    - 更新 `PIPELINE_GAPS.md`，将 `§2.3`、`§2.5`、`§2.7` 回写为 `Closed/Re-scoped`，并同步重写建议收口顺序中的 handoff 说明。
  - 核心决策：
    - 保留 raw MIR codegen 对 `pass MIR Todo` 的最终拒绝，但只把它作为 downstream impossible-state guard；production/materialized 主线必须更早在 verifier/materializer 上失败，不能再把它当 live feature gap 或 production blocker。
    - missing generic template / missing MIR root / concrete-path `TypeKind::Param` 统一冻结为 canonical handoff 错误：前者在 materializer 入口给 source-level hard error，后者在 materialized MIR validation 里直接拒绝；不引入 fallback FQN、默认 type arg 或 late codegen guess。
    - where-bound member call 必须发布和普通 member dispatch 等价的 typed contract。否则 generic `print/println` 与其它 bound-interface 调用即使 typecheck 通过，也会在 HIR stage 因“缺少 typed call-site contract”失败，破坏 materialization/root handoff 的主线验证。
  - 验证结果：
    - `cargo test -p scoopc refactor_materialized_mir`
    - `cargo test -p scoopc codegen_gap_inventory`
    - `cargo test -p scoopc refactor_hir_call_contracts_publish_where_bound_member_dispatch`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/generic_materialization.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/generic_fun_recursion.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/tostring_interface_basic.scoop`
    - `cargo clippy --all-targets -- -D warnings`
    - `rg -c 'TypeKind::Param|MaterializedTodo|MissingMirRootForTemplate|MissingGenericTemplate' crates/scoopc/src`：命中现在集中在 materializer diagnostics/validation（`mir/materialize.rs`: 30）、generic param type-system plumbing（如 `typecheck/lower.rs`: 13、`typecheck/expr/call.rs`: 16、`hir/lower/util.rs`: 12），以及 codegen impossible-state guard（`llvm/codegen/{layout,ty,mod,mir_body,composite_transport,effect_lowered/layout}.rs`: 14 个总命中）；未再发现把这些 bucket 当 production fallback 能力使用的新 active 路径。
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P2 第 5、6 项：materialized MIR handoff 现在把 missing template/root 与 unresolved concrete param 明确冻结为 canonical hard failure；canonical `MaterializedMirPassView` / instance key lookup 继续作为 codegen 主入口，不再依赖 late root guess。
    - `PIPELINE_GAPS.md` 已回写 `§2.3`、`§2.5`、`§2.7` 为 `Closed/Re-scoped`；`§2.3` 仅剩 downstream impossible-state 审计语义，`§2.8` 的 resume-surface 特例继续作为独立 historical bucket 保留。

## P3：收口 raw MIR route 与 call/member contract

### [DONE] P3-T01：收口 raw MIR terminator/call-kind/`PerformResult` route policy

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
    - 更新 `crates/scoopc/src/llvm/codegen/mir_body.rs`，新增 raw MIR route verifier 与 backend-gate helper；在 closure/raw body emission 之前统一拒绝 `Handle` / `ResumeUnwind` / raw `Perform` / raw `PerformResult` / `CallKind::{Virtual,Interface,Resume}`，并删除 `PerformResult` 默认值 fallback，把原来的晚期 `UnsupportedMainBody` 改成 route-bug / missing-handoff-contract guard。
    - 更新 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`，将 `§3.1`、`§3.2`、`§3.3`、`§3.6` 改记为 `P3-T01` owner 的 nonblocking raw-route guard，并补 inventory 断言，冻结新的 trigger 文案。
    - 更新 `crates/scoopc/src/pipeline_gap_audit.rs` 与 `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`，同步 scope-drift baseline、`UnsupportedMainBody` 计数（`mir_body.rs` 321 -> 318，总数 815 -> 812）与 internal bug sentinel 行号基线。
    - 更新 `PIPELINE_GAPS.md`，将 `§3.1`、`§3.2`、`§3.3`、`§3.6` 回写为 `Closed/Re-scoped`，并把建议顺序中的 raw-route 项改成“保持 gate-only 语义”。
  - 核心决策：
    - 不在 raw MIR emitter 内补第二套 handler/resume/cleanup/dynamic-dispatch 语义；这些 shape 统一在 route gate 处 fail-fast，并明确要求走 published late-lowered boundary 或 upstream handoff contract。
    - `PerformResult` 到达 raw emitter 直接视为 route bug；production path 不再制造默认值兜底，从而消除 silent miscompile 风险。
    - `§3.1`、`§3.2`、`§3.3`、`§3.6` 在 inventory 中继续保留 `RawMirLlvm` route，但降为 `production_blocker = false` 的 historical/raw-route guard，表示它们仍需可执行审计，却不再是默认主线的 live backend blocker。
  - 验证结果：
    - `cargo test -p scoopc refactor_llvm_raw_route_gate -- --nocapture`
    - `cargo test -p scoopc raw_mir_effect_control_route -- --nocapture`
    - `cargo test -p scoopc codegen_gap_inventory`
    - `cargo test -p scoopc pipeline_gap_audit`
    - `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
    - `cargo test -p scoopc refactor_llvm_backend_gate`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_direct_handle_resume_emit_llvm.scoop`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P3 第 1 项：raw MIR emitter 现在只接受 raw-safe 输入；effect/control terminator、unsupported call kind 与 `PerformResult` 都在 route gate 或 impossible-state guard 处收口，不再晚到 body emission 才以 unsupported/default-value 形式失败。
    - `PIPELINE_GAPS.md` 已回写 `§3.1`、`§3.2`、`§3.3`、`§3.6` 为 `Closed/Re-scoped`，并明确这些编号现在只表示 raw-route gate / handoff-contract audit，而不再是默认主线的 live blocker。

### [DONE] P3-T02：收口 ctor/default-arg typed contract，删除 backend 补参/猜测

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
    - 更新 `crates/scoopc/src/llvm/codegen/class_ctor.rs`，删除按 ctor arity 选择目标 ctor 的 fallback，以及 `call_info=None` 时自行构造 positional arg mapping 的 fallback；direct ctor call、`super(...)`、`this(...)` 现在只消费已发布的 selected/ordered args contract。
    - 更新 `crates/scoopc/src/llvm/codegen/mir_body.rs`，让 pass-MIR class ctor 路径继承 `CtorCallInfo.arg_mapping.len()` 作为 `ordered_param_count`，不再把 contract 压缩成 `args.len()`。
    - 更新 `crates/scoopc/src/hir/mod.rs` 注释，明确 `ClassInit.ctors` 的用途是执行 published ctor contract，而不是按参数形状临时猜测。
    - 新增 `crates/scoopc/src/pipeline/hir_stage.rs::refactor_hir_ctor_contract_canonicalizes_default_args_to_ordered_slots`、`crates/scoopc/src/pipeline/mir_stage.rs::refactor_mir_ctor_default_args_lower_to_ordered_class_ctor`、`crates/scoopc/src/llvm/tests.rs::refactor_llvm_ctor_default_arg_contract_lowering`，并强化 `refactor_mir_call_contract_lowers_typed_call_sites` 对 top-level/extension default args 的 ordered-args 断言。
    - 更新 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`、`crates/scoopc/src/pipeline_gap_audit.rs` 与 `PIPELINE_GAPS.md`，将 `§3.9`、`§3.10` 回写为 closed/re-scoped 的 typed-contract guard。
  - 核心决策：
    - `CtorCallInfo` 现在是 ctor 选择与参数顺序的唯一权威 handoff。backend 仍可在 mapping 明确标记 `None` 时求值声明处默认值，但这被视为“消费已发布 contract”，不再是“现场补参/猜测”。
    - pass-MIR / refactor-MIR 都统一要求 ctor ordered args contract 由 upstream 明确给出；backend 若再遇到 arity drift，只能作为 contract bug 失败，而不是按参数个数或 call-site 形状自愈。
    - 不为 `P3-T02` 重开新的 MIR snapshot fixture；class ctor default args 的覆盖改放到 HIR/MIR/LLVM 的虚拟源码单测里，避免把无关 golden 基线一并改写。
  - 验证结果：
    - `cargo test -p scoopc refactor_hir_ctor_contract_canonicalizes_default_args_to_ordered_slots`
    - `cargo test -p scoopc refactor_mir_call_contract`
    - `cargo test -p scoopc refactor_mir_ctor_default_args_lower_to_ordered_class_ctor`
    - `cargo test -p scoopc refactor_llvm_call_contract_lowering`
    - `cargo test -p scoopc refactor_llvm_ctor_default_arg_contract_lowering`
    - `cargo test -p scoopc codegen_gap_inventory`
    - `cargo test -p scoopc pipeline_gap_audit`
    - `cargo test -p scoopc llvm_tests`（当前仓库下该过滤串命中 0 tests）
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/call_contracts.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/default_param_call_site_fill_basic.scoop`
    - `cargo clippy --all-targets -- -D warnings`
    - 额外检查：`cargo test -p scoopc llvm::tests` 仍有 8 个与本任务无关的现存失败（closure/function-value/explicit-root-frame 相关），未构成 `P3-T02` 前置阻塞，因此未改动 TODO 顺序。
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P3 第 3、4 项：ctor selected binding 与 default-arg canonicalization 已上移到 typed contract；backend 不再承担补参、arity 容错或 ctor 猜测职责。
    - `PIPELINE_GAPS.md` 已回写 `§3.9`、`§3.10` 为 `Closed/Re-scoped`，并通过 `codegen_gap_inventory.rs` / `pipeline_gap_audit.rs` 将它们冻结为 upstream typed-contract drift guard，而不再是默认主线 live gap。

### [DONE] P3-T03：收口 `StoreMember` continuation route 与 raw function-ref normalization regression

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
    - 更新 `crates/scoopc/src/typecheck/expr/call.rs`，把“调用返回的 callable”分支从直接调用底层 `infer_expr_type(...)` 改为 `inputs.infer(...)`，确保 `make(1)()` / `choose(mode)()` 这类 nested callable callee 的函数类型会写回 `inferred_expr_tys`，不再在 typed HIR lowering 中退成 `Any`。
    - 更新 `crates/scoopc/src/hir/lower/expr.rs`，对保留 member-access 形状的 `String.length()` call 显式保留 callable-typed callee；更新 `crates/scoopc/src/hir/lower/mod.rs`，新增 `typed_hir_preserves_function_typed_nested_call_callee` 与 `typed_hir_top_level_immutable_receiver_closure_keeps_length_as_call_in_side_table` 回归，锁定 nested callable callee 与 top-level immutable receiver closure side table 形状。
    - 更新 `crates/scoopc/src/mir/materialize.rs` 与 `crates/scoopc/src/effect_facts/builder.rs`，让 `MaterializedMir` 显式保留顶层 value 类型索引，并在 effect-facts stage 用该索引为 `topNamed` / `topPatternF` / `topFp` 这类顶层 callable value / `FunPtr` 构建 surface contract，而不再假设 materialized snapshot 里仍留有 generic `InitializerRoot`。
    - 更新 `crates/scoopc/src/llvm/codegen/call/lowering.rs`，把 builtin member-call short-circuit（`length` / `concat` / `toInt` / `hash` / `toString` 等）前移到 generic callable-callee 分支之前，避免 top-level immutable receiver closure 在旧 HIR init path 中把 `this.length` 错当成裸 `MemberAccess` 值去发射。
    - 更新 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`、`crates/scoopc/src/pipeline_gap_audit.rs` 与 `PIPELINE_GAPS.md`，将 `§3.7` 回写为 `P3-T03` owner 的 regression guard，并将 `§3.13` 回写为 closed/re-scoped 的 upstream contract guard。
  - 核心决策：
    - 不在 HIR stage 或 LLVM emitter 上为 nested callable 临时猜类型；直接在 typecheck 阶段补全 `inferred_expr_tys` 发布，使 `make(1)()` / `choose(mode)()` 这类 call-of-call 继续走统一 callable contract 主线。
    - 不把 `String.length()` 改写成新的 synthetic top-level FQN 或额外 wrapper；保持“member-access callee + callable surface”这一既有建模，只修正 typed HIR lowering 与 HIR call lowering 的消费顺序。
    - 对顶层 callable value / `FunPtr` 的 surface contract 不再回 generic MIR 根列表搜索，因为 production materialized snapshot 天然不会保留 `InitializerRoot`；authoritative 信息应来自 materializer 已持有的顶层 value 类型索引。
    - `StoreMember` 的 `Ambiguous` route 本轮不新增 workaround；沿用 production MIR verifier / materialized validation / raw codegen gate 的现有分层约束，并在文档与 inventory 中正式闭合 `§3.13`。
    - `cargo test -p scoopc llvm::tests -- --nocapture` 仍有 3 个现存失败：`closure_call_with_real_outward_effect_uses_explicit_outcome_boundary`、`closure_call_without_outward_effect_stays_on_direct_call_surface`、`managed_function_emits_explicit_root_frame_descriptor`；它们分别落在后续 `P4` callable ABI / adapter 与 explicit-root-frame 相关收尾，不属于 `P3-T03` 的直接 blocker，因此未改动 TODO 顺序。
  - 验证结果：
    - `cargo test -p scoopc refactor_mir_member_access_codegen -- --nocapture`
    - `cargo test -p scoopc refactor_mir_store_member_codegen -- --nocapture`
    - `cargo test -p scoopc typed_hir_preserves_function_typed_nested_call_callee -- --nocapture`
    - `cargo test -p scoopc typed_hir_top_level_immutable_receiver_closure_keeps_length_as_call_in_side_table -- --nocapture`
    - `cargo test -p scoopc materialized_mir_closure_private_symbols_use_stable_hash_namespaces -- --nocapture`
    - `cargo test -p scoopc callable_value_and_top_level_funptr_named_args_keep_binding_order_in_mir -- --nocapture`
    - `cargo test -p scoopc callable_value_pattern_binder_receiver_named_args_fixture_codegen_succeeds -- --nocapture`
    - `cargo test -p scoopc higher_order_aggregate_return_reloads_string_receiver_after_gc_sensitive_arg_eval -- --nocapture`
    - `cargo test -p scoopc higher_order_effectful_function_value_uses_schema_aware_carrier_adapter -- --nocapture`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/assignment_places.scoop`
    - `cargo test -p scoopc llvm::tests -- --nocapture`：当前与本任务直接相关的 5 个历史失败已消失，但仍剩 3 个后续任务范围内的现存失败（见上）。
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P3 第 5-6 项：`StoreMember` continuation route 现在由 upstream MIR verifier / materialized validation 明确要求 `Unique/None`，`Ambiguous` 不再被 LLVM emitter 现场兜底；`§3.7` 继续只保留 regression audit 责任，并且顶层 callable value / `FunPtr` / nested callable callee / pattern binder 路径都已被 run-pass 与 IR 回归重新锁定。
    - `PIPELINE_GAPS.md` 已回写 `§3.7` 与 `§3.13`：前者保持 `Closed/Re-scoped` 且不再是 production blocker，后者从 `Open` 收口为 `Closed/Re-scoped` 的 upstream contract guard。

## P4：收口 effect-refactor ABI、adapter 与 unwind/main 路由

### [DONE] P4-T01：让 actual outward effect set 唯一决定 callable ABI，并补齐 effect-typed callable adapter

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
    - 更新 `crates/scoopc/src/effect_facts/builder.rs`，为 `CallKind::FunValue` 动态调用点补 local callable provenance 解析：现在会优先从 `MakeClosure`、`TopLevelRef`、resolved member fun，以及 direct-call result provenance 恢复真实 callable，再复用 authoritative callable facts / step schema，而不是只按 surface `declared_row` 构造 `DynamicFallback` effect-step 上界。
    - 更新 `crates/scoopc/src/llvm/codegen/mir_body.rs`，让 plain dynamic call 对 closure / function-value 的 actual-outward 判定优先查询 published late-lowered callable ABI；outward-empty callable 即使 surface type 带非 `Pure` effect row，也会留在 plain ABI，actual outward 非空 callable 才继续走 explicit Step boundary 或 adapter guard。
    - 更新 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`、`crates/scoopc/src/pipeline_gap_audit.rs`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 与 `PIPELINE_GAPS.md`，将 `§3.12`、`§5.1`、`§5.4` 回写为 closed/re-scoped 的 effect-routing guard，并同步 inventory / audit / failure-policy 基线。
  - 核心决策：
    - `FunValue` call-site 的 ABI 分类必须先看 authoritative callee provenance，再看 surface function type。否则 `val thunk: () -> Int / Ask = { handle { ... } }` 这类 actual outward-empty callable 会被误发布成 effect-step dynamic invoke contract。
    - plain-path 的 “may outward effect” 判定不能继续依赖保守 summary；优先消费 published late-lowered callable ABI，才能让 `main(args)`、outward-empty closure/function-value 与 effect-typed surface 在同一套 actual-outward routing 下闭合。
    - `§3.12`、`§5.1`、`§5.4` 关闭后仍保留在 inventory 中，但只作为 nonblocking regression / routing guard；后续若回归，只允许以 contract drift / routing bug 重新暴露，而不是恢复成默认主线 live blocker。
  - 验证结果：
    - `cargo test -p scoopc refactor_llvm_function_abi_entry_shells_use_refactor_direct_entry`
    - `cargo test -p scoopc refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry`
    - `cargo test -p scoopc closure_call_without_outward_effect_stays_on_direct_call_surface`
    - `cargo test -p scoopc closure_call_with_real_outward_effect_uses_explicit_outcome_boundary`
    - `cargo test -p scoopc effectful_funptr_call_uses_explicit_outcome_boundary`
    - `cargo test -p scoopc refactor_llvm_no_outward_plain_abi_layout_has_no_step_shell`
    - `cargo test -p scoopc refactor_call_site_facts_distinguish_plain_call_and_effect_adapter_after_solver`
    - `cargo test -p scoopc refactor_effect_solver_consumes_higher_order_function_value_call_in_handle`
    - `cargo test -p scoopc codegen_gap_inventory`
    - `cargo test -p scoopc pipeline_gap_audit`
    - `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_typed_plain_adapter_aggregate_return_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/receiver_function_value_call_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_higher_order_when_direct.scoop`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P4 第 1、2、4、5 项：callable ABI routing 现在由 actual outward effect set / published callable ABI 决定；plain closure / function-value / `FunPtr` 的 effect-typed surface 已通过 adapter 或 published boundary 收口；`main(args)` 的 outward-empty plain routing 保持稳定。
    - `PIPELINE_GAPS.md` 已回写 `§3.12`、`§5.1`、`§5.4` 为 `Closed/Re-scoped`；`codegen_gap_inventory.rs` / `pipeline_gap_audit.rs` 将这三项冻结为 nonblocking effect-routing guard，而 `P4-T02` 继续单独负责 `§5.3` cleanup/unwind contract 收尾。

### [DONE] P4-T02：收口 cleanup/unwind contract 与 `main(args)` plain routing

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
    - 更新 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`，将 `PIPELINE_GAPS §5.3` 从 live production blocker 回写为 `P4-T02` 关闭后的 cleanup/unwind guard，并补充对应 inventory 单测。
    - 更新 `crates/scoopc/src/pipeline_gap_audit.rs`，把 `§5.3` 纳入 codegen scope-drift baseline，确保后续若重新漂回 live blocker 会被审计抓到。
    - 更新 `PIPELINE_GAPS.md` 顶部摘要、`§5.3` 与建议收口顺序，明确 effect-refactor ABI/routing 主线已经闭合，`§5.3`/`§5.4` 仅保留 guard 语义。
  - 核心决策：
    - 不再把 `§5.3` 当作 live implementation gap：现有 lowering/verifier 已经把 cleanup state、origin/resume-state、source slice 与 frame 生命周期固定到 published contract 上，本任务的收尾是把这一事实同步回 executable inventory 与 gap 账本。
    - `main(args)` 的问题继续只通过 outward-empty plain routing 解决；本任务只复核既有 plain-entry 路由与 wrapper 行为，不引入新的 `main` 特判或 Step argv ABI。
  - 验证结果：
    - `cargo test -p scoopc refactor_llvm_resume_unwind_lowering`
    - `cargo test -p scoopc refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry`
    - `cargo test -p scoopc refactor_llvm_main_wrapper_routes_unhandled_outward_to_exit_code`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_raise_cleanup_gc_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_return_from_function_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_return_from_function_finally.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_return_from_function_any_boxing.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/std_process_args_exit_basic.scoop`
    - `cargo test -p scoopc codegen_gap_inventory`
    - `cargo test -p scoopc pipeline_gap_audit`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P4 第 3、4、6 项：`ResumeUnwind` cleanup/unwind contract 与 `main(args)` plain routing 已通过已实现代码和回归验证收口；inventory/gap audit 现在将 `§5.3`、`§5.4` 视为 closed guard，而非默认主线 blocker。
    - `PIPELINE_GAPS.md` 已回写 `§5.3` 为 `Closed/Re-scoped`；`§5.4` 保持 closed 状态并在本任务中完成复核。`PLAN.md` 阶段顺序未改变，因此无需额外修改。

### [DONE] P4-T03：隔离 array literal synthetic helper call-site identity，修复 enum ctor contract 污染

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.13、§4.4、§4.5
- 目标：
  - 让 array literal 合成出的 `__scoop_array_builder_*` helper call 与元素自身的用户 call/ctor call 拥有稳定且互不冲突的 typed call-site identity。
  - 修复 enum variant ctor / tuple payload 等元素表达式在 direct MIR 中被误降成 `__scoop_array_builder_push(...)` 的污染问题。
- 当前实现入口：
  - `crates/scoopc/src/hir/lower/expr.rs::synth_array_lit_from_exprs`
  - `crates/scoopc/src/mir/lower.rs`
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs::refactor_llvm_array_composite_transport`
- 已确认的阻塞症状：
  - 对 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs::array_composite_transport_source` 对应源做 `dump-mir`，现可观察到：array literal 元素 `Hit(Point(...))` / `Pair((...))` 自身会先被误降成单参数 `scoop.core.__scoop_array_builder_push(...)`，返回 `sample.Item`，随后真正的 builder push 又把该错误结果作为元素继续 push。
  - `cargo test -p scoopc refactor_llvm_array_composite_transport` 当前会报 `refactor array_builder_push arg contract`；这是 `P5-T01` 的直接前置阻塞，而不是 backend 可以局部绕过的 symptom。
- 必须实现的内容：
  1. 为 array literal synthetic helper calls 提供稳定、可区分、不会与元素用户表达式复用的 call-site span / identity。
  2. 确保 array literal 元素中的 enum ctor / class ctor / 普通 direct call 继续发布并消费各自的 typed call-site contract，不再被 helper callee/metadata 覆盖。
  3. 为该类污染增加 regression 覆盖。
     - direct MIR 层至少要能断言：真正的 `__scoop_array_builder_push` 仍是双参数 helper call；元素自身的 enum ctor 保持 `Rvalue::EnumVariant` 或正确的 ctor/direct-call 形状，而不是伪装成 helper intrinsic。
     - LLVM stage 回归需要继续覆盖 `refactor_llvm_array_composite_transport`。
- 必须遵从的约束：
  - 不得在 backend 侧接受单参数 `__scoop_array_builder_push`、猜测缺失 builder 参数，或把污染后的 helper call 当成新的 canonical contract。
  - 不得通过缩窄 fixture 形状、绕开 enum ctor、改写元素表示来掩盖上游 contract 污染。
- 验证：
  1. `cargo test -p scoopc refactor_llvm_array_composite_transport`
  2. 推荐新增：`cargo test -p scoopc refactor_mir_array_literal_helper_calls_keep_distinct_call_contracts`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/aggregate_transport.scoop`
- 完成条件：
  - array literal 的 synthetic builder helper 不再污染元素表达式的 typed call-site contract。
  - `P5-T01` 入口所需的 enum ctor / array builder contract 可以作为 authoritative input 稳定到达 composite transport/codegen。
- 依赖：`P4-T02`
- 完成记录：
  - 改动范围：
    - 更新 `crates/scoopc/src/hir/lower/mod.rs`，新增独立的 synthetic helper call-site span 分配器，让 array-builder helper call-site identity 与既有 synthetic local decl span 方案解耦。
    - 更新 `crates/scoopc/src/hir/lower/expr.rs`，让 `build_array_lit_expr(...)` 与 `synth_array_lit_from_exprs(...)` 生成的 `__scoop_array_builder_new/push/build*` helper calls 使用稳定且互不复用的 synthetic call-site span，不再复用元素表达式的用户 span。
    - 更新 `crates/scoopc/src/mir/lower.rs`，新增 `refactor_mir_array_literal_helper_calls_keep_distinct_call_contracts` 回归，直接锁定 `__scoop_array_builder_push` 仍是双参数 helper call，且数组元素里的 enum ctor 继续保持 `Rvalue::EnumVariant`。
    - 更新 `tests/fixtures/mir_refactor/aggregate_transport.mir`，同步 direct MIR golden 到新的 helper call-site identity。
    - 更新 `PIPELINE_GAPS.md`，关闭 `§1.13`，并把 `§4.4` / `§4.5` 的描述收紧为“剩余的是 composite transport/backend gap，而不是上游 helper contract 污染”。
  - 核心决策：
    - 不在 backend 接受单参数 `__scoop_array_builder_push` 或猜测缺失 builder 参数；根因修复必须发生在 typed HIR call-site identity 发布阶段。
    - synthetic helper call-site span 使用独立计数器与独立 span 区间，而不是复用元素 span，也不与 synthetic local decl span 共用同一分配器；这样既消除 helper/元素 contract 覆盖，又避免无关 local identity 策略被一起改写。
    - direct MIR 继续作为最早的可执行防线：数组元素中的 enum variant ctor 若再被 helper contract 污染，应先在 MIR 回归里暴露，而不是等 composite transport/LLVM 才出现 `array_builder_push` 症状。
  - 验证结果：
    - `cargo test -p scoopc refactor_mir_array_literal_helper_calls_keep_distinct_call_contracts -- --nocapture`
    - `cargo test -p scoopc refactor_mir_aggregate_transport_records_composite_contracts -- --nocapture`
    - `cargo test -p scoopc refactor_llvm_array_composite_transport -- --nocapture`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/aggregate_transport.scoop`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P5 的前置收口要求：array literal synthetic helper call-site pollution 已被前移修复，`P5-T01` 现在可以拿到未被 helper 覆盖的 enum ctor / tuple payload / array builder contract 作为 authoritative input。
    - `PIPELINE_GAPS.md` 已回写 `§1.13` 为 `Closed/Re-scoped`；`§4.4` / `§4.5` 保持 `Open`，但其描述现在只再表示真实 composite transport/backend residual，而不再夹带上游 call-site identity 污染。

## P5：统一 aggregate / composite transport

### [DONE] P5-T01：统一 composite transport contract，关闭 enum/array boxing residual

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
- 前置已闭合：
  - `P4-T03` 已修复 array literal synthetic helper call-site 污染；本任务入口现在可以稳定拿到未被 helper 覆盖的 enum ctor / composite element contract。
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
- 依赖：`P4-T03`
- 完成记录：
  - 改动范围：
    - 更新 `crates/scoopc/src/llvm/codegen/composite_transport.rs`，新增统一的 composite contract backend guard helper，供 LLVM lowering 各入口在 contract 漂移时回报稳定的 `BackendGateError`，不再把这些 residual 暴露成用户可见 `UnsupportedMainBody`。
    - 更新 `crates/scoopc/src/llvm/codegen/mir_body.rs`，将 composite value erasure / descriptor publication 的 residual 从 `UnsupportedMainBody` 改为 `PIPELINE_GAPS §4.1` backend guard，明确 tuple/struct/enum payload -> `Any`/`Ref` 现在依赖单一 descriptor-backed boxing contract。
    - 更新 `crates/scoopc/src/llvm/codegen/enum_lowering.rs` 与 `crates/scoopc/src/llvm/codegen/control_flow.rs`，将 oversized int payload、nested enum payload、tuple/struct payload、多字段 inline payload 的残余失败改写为 `§4.3` / `§4.4` contract guard；这些形状现在必须在 enum layout 阶段先切到 boxed composite transport。
    - 更新 `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`，将 array get/set composite metadata 缺失、operation 漂移、element layout 漂移、composite value 落回 `u64` decode 的 residual 改写为 `§4.5` contract guard，并保持 cross-thread resume composite payload 继续复用同一 descriptor helper。
    - 更新 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`、`crates/scoopc/src/pipeline_gap_audit.rs`、`crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 与 `PIPELINE_GAPS.md`，把 `§4.1`、`§4.3`、`§4.4`、`§4.5` 从旧的 CG-T04 blocker/partial 语义回写为 `P5-T01` owner 的 closed/re-scoped guard，并同步失败策略/审计基线。
  - 核心决策：
    - 不为 enum、array、value boxing、cross-thread resume payload 分别维护独立 contract；继续复用同一个 descriptor-backed composite transport helper，并把各入口的 residual 统一降为 backend contract guard。
    - 不删除 downstream guard；保留它们作为 impossible-state 防线，但 guard 文案不再表达“尚未支持某特性”，而是明确说明 boxed payload / array metadata / value erasure contract 已被上游破坏。
    - `§4.1` / `§4.3` / `§4.4` / `§4.5` 关闭后，inventory 继续保留这些稳定 gap id 作为 regression / drift audit，但不再允许它们以 production blocker 身份出现在 active codegen inventory 中。
  - 验证结果：
    - `cargo test -p scoopc codegen_gap_inventory -- --nocapture`
    - `cargo test -p scoopc pipeline_gap_audit -- --nocapture`
    - `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
    - `cargo test -p scoopc refactor_llvm_composite_transport_contract_emits_layout_descriptor_globals -- --nocapture`
    - `cargo test -p scoopc refactor_llvm_value_boxing_transport -- --nocapture`
    - `cargo test -p scoopc refactor_llvm_enum_payload_transport -- --nocapture`
    - `cargo test -p scoopc refactor_llvm_array_composite_transport -- --nocapture`
    - `cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport -- --nocapture`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/aggregate_transport.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_payload_boxing_any_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_oversized_variant_boxing_suppressed.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/option_nested_custom_enum_payload_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop`
    - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_composite.scoop`
    - `cargo clippy --all-targets -- -D warnings`
    - `rg 'enum payload larger than word|nested enum/tuple/struct payload unsupported|array composite element u64 word unsupported|value boxing tuple/struct unsupported' crates/scoopc/src`：0 命中。
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P5 第 1 项：enum payload boxing、array composite element transport、value boxing erasure 与 cross-thread resume composite payload 已统一到同一套 descriptor-backed composite transport contract；不再保留“enum 一套 / array 一套 / effect payload 一套”的 live blocker 语义。
    - `PIPELINE_GAPS.md` 已回写 `§4.1`、`§4.3`、`§4.4`、`§4.5` 为 `Closed/Re-scoped`，并同步将它们在 `codegen_gap_inventory.rs` / `pipeline_gap_audit.rs` 中冻结为 `P5-T01` owner 的 closed guard。阶段顺序未改变，因此无需更新 `PLAN.md`。

### [DONE] P5-T02：收口 closure env/capture transport 与 pattern `is Type` residual

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
    - 更新 `crates/scoopc/src/typecheck/when_pat.rs` 与 `crates/scoopc/src/typecheck/expr/error.rs`，为 `when` 的 `is Type` pattern 新增前端 gate：dynamic value-type target、pure function-type target、effectful function-type target 不再流到 LLVM backend unsupported，而是以前端明确诊断拒绝。
    - 更新 `crates/scoopc/src/mir/lower.rs`，收紧 runtime type static fold：`value vs ref`、`class/string/function/union ref vs value` 等显然不可能的 pattern 现在直接折叠为 `AlwaysFalse`，不再晚到 runtime type test。
    - 更新 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`，补 static-false IR 断言，固定 disjoint value/ref pattern `is Type` 不再走 runtime type test。
    - 新增 `tests/fixtures/typecheck/when_is_pattern_{dynamic_value_runtime_test,function_type,effectful_function_type}_is_error.scoop`，固定新的前端拒绝 surface。
    - 更新 `tests/fixtures/runtime_gc/gc_move_enum_maybe_ref_closure_capture_basic.scoop`，删除 `Box` workaround，改为 direct enum capture，验证 closure env/capture transport 在 moving GC 下直接承载 aggregate capture。
    - 更新 `PIPELINE_GAPS.md`、`crates/scoopc/src/llvm/codegen_gap_inventory.rs`、`crates/scoopc/src/pipeline_gap_audit.rs`，将 `§3.8`、`§3.11` 回写为 closed guard / frontend gate 状态，并同步 inventory owner、route、audit baseline。
  - 核心决策：
    - `when` 的 `is Type` pattern 只保留两类默认主线路径：类/接口/String runtime test，以及可静态判定的 value pattern。其余仍未开放的 dynamic value-type / function-type target 不再让 backend 暴露 `UnsupportedMainBody`，而是前移为明确 `FrontendReject`。
    - `runtime_type_static_fold(...)` 不再只处理“同型真 / value-value 假”的最小子集；对 `value vs ref` 等显然不可能的组合，直接在 MIR 元数据层冻结为 `AlwaysFalse`，避免 backend 为不必要的 runtime check 承担 residual gap。
    - closure env 继续以 `Unit` / `Tuple` 作为上游 MIR 不变量，但默认主线接受的 aggregate capture 与 mutable capture box 全部复用 P5-T01 的 descriptor-backed composite transport contract；因此 runtime GC 回归必须删除 `Box` workaround，改测 direct aggregate capture。
  - 验证结果：
    - `cargo test -p scoopc refactor_llvm_closure_env_transport`
    - `cargo test -p scoopc refactor_llvm_runtime_type_primitives`
    - `cargo test -p scoopc codegen_gap_inventory`
    - `cargo test -p scoopc pipeline_gap_audit`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/pattern_is_type.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_trace_closure_capture_string_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_move_enum_maybe_ref_closure_capture_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/parameterized_supertype_interface_dispatch.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_is_pattern_dynamic_value_runtime_test_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_is_pattern_function_type_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_is_pattern_effectful_function_type_is_error.scoop`
    - `cargo clippy --all-targets -- -D warnings`
    - `cargo fmt`
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P5 第 3、5、6 项：closure env/capture transport 已与 composite transport contract 收口到同一套 descriptor/trace 规则；pattern `is Type` residual 已通过“实现默认主线路径 + 前移未开放 surface”闭合，不再保留 raw MIR backend unsupported。
    - `PIPELINE_GAPS.md` 已回写 `§3.8`、`§3.11` 为 `Closed/Re-scoped`，并同步将对应 codegen inventory 条目标记为 `P5-T02` owner 的 closed guard / frontend gate；阶段顺序未改变，因此无需更新 `PLAN.md`。

## P6：同步 frontend gate、收尾 partial surface、重写账本

### [DONE] P6-T01：收尾 `§3.5` / `§7.6` partial surface，统一 runtime cast 与 GC pin/handle policy

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
    - 更新 `crates/scoopc/src/typecheck/expr/error.rs`、`crates/scoopc/src/typecheck/expr/call.rs`、`crates/scoopc/src/pipeline/hir_stage.rs`、`crates/scoopc/src/hir/lower/mod.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`，把 GC pin/handle 的前端 gate、typed HIR call contract、`UIntPtr` lowering 与 pure/effect-refactor LLVM intrinsic lowering 收口到同一条正式主线。
    - 更新 `PIPELINE_GAPS.md`、`crates/scoopc/src/llvm/codegen_gap_inventory.rs` 与 `crates/scoopc/src/pipeline_gap_audit.rs`，将 `§3.5`、`§7.6` 从 `Partial` 回写为 `Closed/Re-scoped`，并把 inventory/audit 语义收紧为 closed guard / frontend gate。
    - 新增 `crates/scoopc/src/pipeline/hir_stage.rs::refactor_hir_gc_intrinsic_member_calls_publish_intrinsic_contracts`、`crates/scoopc/src/pipeline/mir_stage.rs::refactor_mir_gc_handle_raw_uintptr_token_stays_scalar`；新增 `tests/fixtures/typecheck/gc_{unpin_requires_pinned,handle_get_requires_handle,handle_drop_requires_handle}_is_error.scoop`；更新现有 GC runtime/typecheck fixtures 的文案与 `GcHandle.raw: UIntPtr` token 形状。
  - 核心决策：
    - `§3.5` 不再保留“后端半支持”状态：默认主线允许的 runtime `is/!is/as/as?` 继续走统一的 MIR metadata + LLVM lowering；函数类型 / effectful function-type cast 维持前端明确拒绝，不再把剩余 surface 留成 partial bucket。
    - `§7.6` 采用“保留已实现能力 + 前移未开放 surface”的收口方式，而不是缩小默认主线：`GC.pin/unpin`、`GC.handleNew/Get/Drop`、`GcHandle.raw: UIntPtr` callback/native token round-trip 保持正式支持；值类型 `pin/handleNew`、非 `Pinned` 的 `unpin`、非 `GcHandle` 的 `handleGet/drop`，以及把 `Pinned` 当 ordinary `@Extern` token 的用法统一以前端诊断拒绝。
    - 对任务中暴露的两个阻塞缺口不做 workaround：一是 HIR stage 现在直接为保留 member-access 形状的 GC intrinsic call 发布 typed intrinsic contract；二是 `UIntPtr` 在 typed HIR 中直接按 word-sized scalar lowering，不再落成 ref nominal 后再靠后端修补。
  - 验证结果：
    - `cargo test -p scoopc refactor_hir_gc_intrinsic_member_calls_publish_intrinsic_contracts`
    - `cargo test -p scoopc refactor_mir_value_primitives`
    - `cargo test -p scoopc refactor_mir_gc_handle_raw_uintptr_token_stays_scalar`
    - `cargo test -p scoopc refactor_llvm_runtime_type_primitives`
    - `cargo test -p scoopc codegen_gap_inventory`
    - `cargo test -p scoopc pipeline_gap_audit`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/runtime_typecheck_cast.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_closed_pure_asq_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_effectful_asq_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_effectful_as_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/unsafe_nogc/gc_pin_value_type_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/gc_handle_new_value_type_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/gc_unpin_requires_pinned_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/gc_handle_get_requires_handle_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/gc_handle_drop_requires_handle_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_gc_handle_raw_token_roundtrip_ok.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_pinned_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_pin_unpin_basic.scoop`
    - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_pin_unpin_move_stress_matrix.scoop`
    - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop`
    - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_handle_token_roundtrip_callback_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_handle_stale_callback_token_is_error.scoop`
    - `cargo clippy --all-targets -- -D warnings`
    - `rg '状态：`Partial`' PIPELINE_GAPS.md`：命中仅剩状态定义行，active gap 条目已无 `Partial`。
    - `rg 'refactor value primitive runtime cast unsupported|GC pin/handle intrinsic frontend diagnostic|TODO T1008' crates/scoopc/src tests/fixtures PIPELINE_GAPS.md TODO.md`：0 命中。
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P6 第 2 项：`§3.5` 与 `§7.6` 已不再保留 `Partial`；runtime cast/typecheck 与 GC pin/handle 的默认主线支持面、前端 gate、MIR contract、LLVM lowering、runtime regression 现在彼此一致。
    - `PIPELINE_GAPS.md` 已回写 `§3.5`、`§7.6` 为 `Closed/Re-scoped`；`codegen_gap_inventory.rs` / `pipeline_gap_audit.rs` 现将它们分别冻结为 runtime-cast contract guard 与 GC intrinsic support-surface gate。`PLAN.md` 阶段顺序未改变，因此无需额外修改。

### [DONE] P6-T02：同步 `FrontendReject` surface：or-pattern binder / function-type cast / use-site effect row / struct mutable field

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
    - 更新 `crates/scoopc/src/typecheck/expr/error.rs`、`crates/scoopc/src/typecheck/lower.rs`、`crates/scoopc/src/typecheck/structs.rs`，统一 `or-pattern binder`、function-type `as/as?`、非法 use-site effect row target、`struct var` 的诊断文案，全部改成“当前语言 contract 下非法/不接受该源码形状”的表述。
    - 更新 `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`，同步新的前端拒绝文案、`use_site_eff_arg_not_allowed` 的审计标记，并回写 internal bug sentinel 行号基线。
    - 新增 `tests/fixtures/typecheck/use_site_eff_arg_target_without_eff_param_is_error.scoop`，并同步更新 `when_or_pattern_variant_payload_binder_{is_error,sharing_is_error}.scoop`、`fn_type_cast_{closed_pure_asq,effectful_as,effectful_asq}_is_error.scoop`、`struct_{primary_ctor_var,field_must_be_val}_is_error.scoop` 的预期文案。
    - 更新 `PLAN.md`、`PIPELINE_GAPS.md` 与 `crates/scoopc/src/cone/pre_specialize.rs` 注释，把 `§7.3` 从过时的整体 `FrontendReject` 叙述改写为“名义类型 `Type<eff Row>` 已支持；非法 target 仍以前端诊断拒绝”的真实状态。
  - 核心决策：
    - 保持 `§7.1`、`§7.2`、`§7.5` 为 `FrontendReject`，但要求用户可见文案统一明确指向“当前语言 contract 下非法输入”，不再使用“后端尚未支持”语气。
    - 将 `§7.3` 重新分类为 `Closed/Re-scoped`：仓库中已有 typecheck / infer / run-pass 覆盖证明 `Type<eff Row>` 在声明了 effect row 形参的名义类型上是 production surface；保留的 `use_site_eff_arg_not_allowed` 只用于拒绝把 `eff ...` 填给 builtin / typealias / 无 effect-row 形参的类型。
    - 不为 `§7.3` 新增 backend workaround；继续让 class `var` property 保持支持、`struct` mutable field 保持前端拒绝、function-type cast 保持在 MIR 前被挡住，避免出现“前端已放行，后端再 unsupported” 的分裂状态。
  - 验证结果：
    - `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
    - `cargo test -p scoopc refactor_mir_value_primitives_reject_unsupported_function_type_cast_before_mir`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_sharing_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_effectful_asq_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_effectful_as_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_closed_pure_asq_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/parse/type_args_eff_use_site_order_fail.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/use_site_eff_arg_target_without_eff_param_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/eff_row_param_infer_from_nominal_ok.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/struct_property_setter_not_allowed_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/struct_primary_ctor_var_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/struct_field_must_be_val_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/class_var_property_reassign_ok.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `PIPELINE_GAPS.md` 对应闭合：
    - 对应 `PLAN.md` P6 第 1 项：`§7.1` / `§7.2` / `§7.5` 的前端 gate 文案与实际行为已统一，`§7.3` 则按真实能力改写为正式支持面，不再伪装成整体 reject。
    - `PIPELINE_GAPS.md` 已回写 `§7.3` 为 `Closed/Re-scoped`，并同步更新 `§2.6`、`§8` 中对 effect-row use-site surface 的描述：当前默认主线允许名义类型 `Type<eff Row>`，剩余非法 target 继续由 typecheck 明确拒绝，而不是留给 MIR/backend 暴露 unsupported。

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
