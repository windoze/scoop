# TODO（P6：LLVM codegen clean refactor Part 3 / 待完成任务）

> 生成时间：2026-05-04  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.6  
> 计划基线：[`PLAN.md`](./PLAN.md) §2/P6  
> 前置条件：`TODO-P6-part1.md` 已完成；`TODO-P6-part2.md` 中 `P6-T02m` ~ `P6-T02qg` 已完成；旧 `P6-T03` 单体推进暴露出 backend 与 legacy codegen 边界不清的问题。  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：按 clean refactor backend 原则完成 P6。refactor LLVM backend 拥有整个 function protocol，只复用 effect-neutral value/expression primitive；禁止把旧 statement/function/control-flow codegen 胶合进新 body emitter。

## 全局约束

- [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 是本阶段唯一设计基线；若实现过程中需要改变主张，必须先回写该文档，再继续实现。
- `P6` 不重新打开 P0-P5 已闭合的 selector、typed HIR、direct-style MIR、effect facts、late-lowered state graph、frame lifting 与 schema identity 设计。
- refactor LLVM backend 的 authoritative 输入固定为 P5/P6 handoff：late-lowered state graph、frame schema、boundary map、resume-state map、ABI query、source value ABI、runtime/GC contract。
- 明确禁止：在 refactor backend 中把 legacy `codegen_mir_statement`、旧函数 ABI、旧 call dispatcher、旧 return lowering、legacy handler-stack/outcome 当作通用 fallback。
- 允许共享：target/pipeline/runtime symbol 等基础设施，以及不携带 effect/control/ABI 决策的 value/expression primitive。
- 若旧 helper 需要复用，必须先抽成不知道新旧线路的纯 primitive；否则在 refactor 路径中重建。
- 禁止 silent skip：source slice 中每条 statement 要么被显式 lower，要么被 published contract 消费，要么 fail fast。
- 禁止用 `unreachable`、默认零值、null payload 或 raw `malloc` 代表尚未建模的合法运行时语义。
- 本阶段仍不切默认主线，不运行 full regression；所有端到端验证必须通过 `--effect-pipeline refactor` 或共享同一 refactor stage helper 的测试入口。

## 迁移说明

- 从 [`TODO-P6-part2.md`](./TODO-P6-part2.md) 迁移的 required blocker：`P6-T02qh`。
- 执行 `P6-T02qh` 验证时新增前置 blocker：`P6-T02qga`。
- [`TODO-P6-part2.md`](./TODO-P6-part2.md) 中旧 `P6-T03` 单体任务已废弃，改由本文件的 `P6-T03a` ~ `P6-T03i` 与 `P6-T03R` 承接。
- [`TODO-P6-part2.md`](./TODO-P6-part2.md) 中旧 `P6-T04` / `P6-T04R` / `P6-T05` / `P6-T05R` 迁移到本文件，并按 clean backend 边界更新依赖。

## [DONE] P6-T02qga：发布 call-boundary 本地消费 outward case 的 continuation composition contract，禁止 escaped continuation 绕过 callee resume body

- 来源：执行 `P6-T02qh` 验证时发现的前置 blocker。
- 参考：
  - [`TODO-P6-part2.md`](./TODO-P6-part2.md) `P6-T02o`, `P6-T02qd`, `P6-T02qg`
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.5, §5.6
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout,body}.rs`
- 背景：
  - `P6-T02qh` 的 `effect_multi_escape_indirect_direct_while.scoop` run-pass 验证要求 escaped `Continuation<String, Int>` resume 时先恢复产生 outward case 的 callee continuation，再回到 caller boundary resume state。
  - 当前 refactor body emitter 在 `Call` boundary 的 outward case 被本地 handle arm 消费时，已经能从 callee `Step` 拆出 continuation，但后续 `emit_or_consume_outward_case` 丢弃该 callee continuation 并只创建 caller continuation。
  - 结果是 `k.resume("alpha")` 直接把 caller call-boundary result 设为 `alpha`，跳过 `fetch` 的 `fetch_resume` 路径；这会使 `P6-T02qh` 的 wrapper completion payload projection 即使正确发布，也无法通过真实 run-pass 验证。
- 必须实现的内容：
  1. 发布 call-boundary outward case 被本地消费时的 authoritative continuation composition handoff，明确 callee continuation、caller resume state、payload/result home slot 的关系。
  2. ABI materialization 校验该 composition contract 与 callee `Step_F` case layout、caller boundary result binding、frame/home slot、source type 一致。
  3. refactor body emitter 在本地 handle arm 捕获 continuation 时消费 published composition contract，构造能先 resume callee continuation、再回到 caller resume state 的 escaped continuation。
  4. 对缺失 composition、callee/caller continuation 类型漂移、result slot 漂移或多义 composition fail fast。
  5. 补充定向测试，覆盖 direct call outward 被本地 handle arm 消费、resume 后执行 callee resume body、再回 caller resume state 的路径。
- 必须遵从的约束：
  - 禁止在 backend 中丢弃 callee continuation 后只创建 caller continuation。
  - 禁止回 raw MIR/source shape、site 名称或语句顺序现场猜测 continuation composition。
  - 禁止复用 legacy handler-stack/TLS continuation token 作为 refactor correctness 前提。
- 验证：
  - `cargo test -p scoopc refactor_effect_lowered_call_boundary_continuation_composition`
  - `cargo test -p scoopc refactor_llvm_call_boundary_continuation_composition`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
- 完成条件：
  - call-boundary outward case 本地消费时，escaped continuation 会先恢复 callee continuation，再回到 caller boundary resume state。
  - `effect_multi_escape_indirect_direct_while.scoop` 中 `fetch_resume` / caller tail / wrapper completion payload 的顺序与 golden 一致。
  - `P6-T02qh` 可以只处理 wrapper completion payload projection，不再被 call-boundary continuation composition 缺口阻塞。
- 依赖：P6-T02o，P6-T02qd，P6-T02qg
- 完成记录：
  - 已在 late-lowered handoff 中为 call-boundary outward forwarding 发布 `LateLoweredCallBoundaryContinuationComposition`，记录 callee continuation、caller continuation、caller resume state 与 caller result local/home slot。
  - ABI materialization 现在校验 composition 与 callee `Step_F` case、callee surface resume ABI、caller dispatch forwarding、caller result binding/frame home 和 source type 一致，并对缺失或多义 composition fail fast。
  - refactor LLVM body emitter 在 call-boundary outward case 被本地 handle arm 捕获或继续向外投影时会保留 callee continuation，构造 composed caller continuation；surface resume 该 continuation 时先调用 callee resume，再按 caller call-boundary dispatch 回写 result 并回到 caller resume state。
  - 同步修复 `Step_F` union storage 的 by-value ABI 字节保真问题，避免 Complete(String) 被最大 case layout 误解释；同时放宽 wrapper completion 对 sibling handle arm 的 span-only payload source drift，保证 composed continuation 完成后能返回 resume caller 而不是重跑 owner tail。
  - 验证通过：`cargo test -p scoopc refactor_effect_lowered_call_boundary_continuation_composition`；`cargo test -p scoopc refactor_llvm_call_boundary_continuation_composition`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`；`cargo clippy --all-targets -- -D warnings`（Rust clippy 通过，构建脚本仍输出 macOS SDK 对 `getsectbynamefromheader_64` 的 C deprecation warning）。

## [DONE] P6-T02qh：发布 surface-resume wrapper completion payload projection contract，禁止 P6-T03 在 owner-step `Complete` 投影时发明 wrapper answer 值

- 来源：从 [`TODO-P6-part2.md`](./TODO-P6-part2.md) 迁移。
- 参考：
  - [`TODO-P6-part2.md`](./TODO-P6-part2.md) `P6-T02qc`, `P6-T02qg`
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.2-§5.3.6, §5.5, §5.6, §8
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout,body}.rs`
- 背景：
  - `P6-T02qc` 已发布 shared surface-resume wrapper 的 owner-step -> wrapper-step case 投影。
  - `P6-T02qg` 已发布 callable `Complete(answer)` 的 payload source。
  - `effect_multi_escape_indirect_direct_while.scoop` 暴露真实 `owner_answer_ty=Unit`、`wrapper_answer_ty=Int` 的 projection；wrapper `Complete(Int)` 不能由 backend 默认填值或回 raw MIR/source shape 恢复。
- 必须实现的内容：
  1. 扩展 `LateLoweredSurfaceResumeWrapperCompleteProjection` 或等价 handoff，发布 wrapper complete payload source。
  2. ABI materialization 校验 owner complete payload、wrapper payload source、`Step_F` complete layout、frame/home slot 与 source type。
  3. owner trampoline / wrapper projection complete 分支消费 published payload source。
  4. 对缺失、歧义或类型漂移 fail fast。
  5. 补充定向测试，覆盖 `owner Unit -> wrapper Int`、owner/wrapper type 相同、缺失或漂移三类路径。
- 必须遵从的约束：
  - 禁止把 wrapper `Complete(answer)` 默认成零值或 fixture 私有常量。
  - 禁止让 P6 回 raw MIR/source shape 或按 resume site 名称/顺序恢复 answer source。
  - 禁止把 wrapper surface symbol 退化成 owner-private special case。
- 验证：
  - `cargo test -p scoopc refactor_effect_lowered_surface_resume_wrapper_completion`
  - `cargo test -p scoopc refactor_llvm_surface_resume_wrapper_completion`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
- 完成条件：
  - surface-resume wrapper complete projection 已 authoritative 发布 wrapper payload source。
  - `P6-T03*` 可以只消费 state graph / ABI query 构造 wrapper `Complete(answer)`。
  - 缺失、歧义或类型漂移时在 P5/P6 handoff 边界显式拒绝。
- 依赖：P6-T02qc，P6-T02qg，P6-T02qga
- 完成记录：
  - 已发布 surface-resume wrapper complete payload source handoff：同型 owner/wrapper answer 复用 owner `Complete` payload，`owner Unit -> wrapper Int` 等非同型路径引用 published handle-arm completion payload source。
  - ABI materialization 现在校验 wrapper answer type、payload source type、`Step_F` complete layout 与 frame/home slot；缺失、Unit/non-Unit 漂移和类型漂移会 fail fast。
  - refactor LLVM owner trampoline / wrapper projection complete 分支消费 published payload source，构造 wrapper `Complete(answer)` 时不再默认填值或回 raw MIR/source shape 恢复 answer。
  - 前置 blocker `P6-T02qga` 已完成，`effect_multi_escape_indirect_direct_while.scoop` 的 composed continuation 路径现在会先执行 callee resume body，再回 caller resume state 并投影 wrapper completion payload。
  - 本轮同时消除 macOS SDK 对 runtime stackmap section lookup 的 C deprecation warning：`scoop_stackmap.c` 改用 `getsectiondata()`，并补齐 `scoop_runtime_error_fatal` runtime ABI allowlist，保证相关验证无警告、无 allowlist 失败。
  - 验证通过：`cargo test -p scoopc refactor_effect_lowered_surface_resume_wrapper_completion`；`cargo test -p scoopc refactor_llvm_surface_resume_wrapper_completion`；`cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`；`cargo test -p scoop_runtime`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P6-T03a：固化 clean refactor LLVM backend 边界，抽出 effect-neutral value/expression primitive

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.6
  - [`PLAN.md`](./PLAN.md) §0, §2/P6
  - `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
- 目标：
  - 把 refactor backend 与 legacy statement/function codegen 的边界固化到代码结构中。
  - 允许复用的旧逻辑只能是 effect-neutral value/expression primitive。
- 必须实现的内容：
  1. 盘点 refactor backend 当前调用的 legacy helper，并分类为 primitive 可共享、必须复制/重建、必须移除三类。
  2. 抽出或建立 value/expression lowering API，覆盖 literal、local load/store、tuple/scalar pack/unpack、primitive ops、cast、member read/write primitive。
  3. API 必须由调用方显式传入 type/layout/ABI contract，不得自己决定 call target、return path、boundary dispatch、continuation semantics。
  4. 在 refactor backend 中移除对 statement-level/function-level legacy fallback 的直接依赖，或加 fail-fast guard 防止误用。
  5. 新增审计测试或 grep-based 断言，证明 refactor 主实现不再把 legacy statement/function codegen 当 fallback。
- 必须遵从的约束：
  - 禁止在 shared primitive 中携带 pipeline selector 或新旧线路分支。
  - 禁止 primitive 通过 `Span`、HIR node、旧 effect side table、raw MIR shape 恢复 effect/control 语义。
  - 若某旧 helper 隐含旧 ABI 或旧 control flow，必须复制/重建而不是共享。
- 验证：
  - `cargo test -p scoopc refactor_llvm_clean_backend_boundary`
  - `cargo test -p scoopc refactor_llvm_value_primitive`
  - `rg "codegen_mir_statement|build_unified_lowering_contract|effect_analysis_ctx|effect_op_call_sites|scoop_effect_handler_stack|scoop_effect_outcome" crates/scoopc/src/llvm/codegen/effect_refactor crates/scoopc/src/effect_refactor_pipeline`
- 完成条件：
  - clean backend 的代码边界已落地。
  - 后续 `P6-T03*` 只能通过 published contract + value/expression primitive lower body。
- 依赖：P6-T02qh
- 完成记录：
  - 2026-05-04：完成 clean backend 边界收口。新增 `crates/scoopc/src/llvm/codegen/effect_refactor/value.rs`，作为 refactor backend 唯一允许共享的 effect-neutral value primitive facade；它覆盖 literal/local load-store/source operand lowering、scalar/tuple ABI pack/unpack、非控制流 cast、primitive op 与 canonical MIR member read/write primitive，并在模块边界明确禁止 call target、return path、boundary dispatch、continuation 语义决策。
  - 2026-05-04：`crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 的 source-slice fallback 已改为经 `RefactorValuePrimitives` 调用 `codegen_mir_effect_neutral_statement`；`body.rs` 不再直接调用 legacy statement-level `codegen_mir_statement`，也不再用 legacy direct-call helper lowering refactor print 路径。非中立 call / closure carrier / class ctor / boundary payload / runtime-control cast 会 fail fast，交给后续 `P6-T03b+` 的 published contract 与分类任务闭合。
  - 2026-05-04：在 `crates/scoopc/src/llvm/codegen/mir_body.rs` 抽出 `codegen_mir_effect_neutral_statement` / `codegen_mir_effect_neutral_rvalue`，把可共享的 MIR value/expression lowering 与旧 function/statement/control-flow lowering 分开；新增审计测试锁定 refactor body 不再含旧 statement/function fallback 入口。
  - 2026-05-04：`PLAN.md` 无需改动。
- 已运行验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc refactor_llvm_clean_backend_boundary`
  - `cargo test -p scoopc refactor_llvm_value_primitive`
  - `rg "codegen_mir_statement|build_unified_lowering_contract|effect_analysis_ctx|effect_op_call_sites|scoop_effect_handler_stack|scoop_effect_outcome" crates/scoopc/src/llvm/codegen/effect_refactor crates/scoopc/src/effect_refactor_pipeline`（无 statement/function fallback 命中；仅剩 typed HIR/ABI test scaffolding 中既有 `effect_op_call_sites` 查询）
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
  - 额外非必需检查：`cargo test -p scoopc refactor_llvm_` 仍有既有 layout/stage 测试失败，失败点不来自本任务新增的 clean backend boundary 测试。

## [DONE] P6-T03b：发布 source-slice statement classification contract，禁止 body emitter 静默 skip 或回 raw shape 猜语义

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.6.3
  - `crates/scoopc/src/effect_lowered/{ir,materialize,dump}.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout,body}.rs`
- 目标：
  - 每条 late-lowered source-slice statement 都有明确用途。
  - body emitter 不再靠 ad-hoc `continue` 跳过 `Resume` / `Virtual` / `Interface` call、TopLevelRef、boundary anchor、payload injection 等语句。
- 必须实现的内容：
  1. 为 source-slice statement 发布分类 contract。
  2. 至少区分 effect-neutral value statement、boundary-consumed anchor、resume payload injection、boundary result/completion payload injection、handle synthetic carrier/binder、elided/unreachable、unsupported。
  3. 在 late-lowered dump 中渲染分类结果，方便 fixture 锁定。
  4. ABI/materialization 或 verifier 校验分类与 boundary/source contract 一致。
  5. refactor body emitter 只消费分类结果；遇到未分类或 unsupported statement 必须 fail fast。
- 必须遵从的约束：
  - 禁止在 body emitter 中重新扫描 raw statement kind 来决定是否跳过。
  - 禁止因为当前 fixture shape 简单就把 skip 规则固化在 backend 私有代码中。
- 验证：
  - `cargo test -p scoopc refactor_effect_lowered_source_slice_classification`
  - `cargo test -p scoopc refactor_llvm_source_slice_classification`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
- 完成条件：
  - source-slice statement 用途已成为 published handoff。
  - body emitter 不再有 silent skip 作为 correctness 前提。
- 依赖：P6-T03a
- 完成记录：
  - 2026-05-04：新增 `LateLoweredCallable.source_statement_classifications` published handoff，为每个 source-slice statement 发布 `effect-neutral`、boundary-consumed anchor、resume payload injection、boundary/result/completion payload injection、handle synthetic binder、elided/unreachable 或 unsupported 分类。
  - 2026-05-04：late-lowering materialization 现在生成分类并校验 source-slice 覆盖、boundary statement anchor 唯一性与 resume payload binding 一致性；stable dump 渲染 `statement_classification`，便于 fixture / snapshot 锁定。
  - 2026-05-04：refactor LLVM ABI materializer 增加 classification handoff verifier；body emitter 移除私有 skip heuristic，只按 published classification lower、skip 或 fail fast，未分类 / unsupported statement 不再静默跳过。
  - 2026-05-04：`EFFECT_REFACTOR.md` §5.6.3 已补充 `LateLoweredCallable.source_statement_classifications` contract 说明。
  - 验证通过：`cargo test -p scoopc refactor_effect_lowered_source_slice_classification`；`cargo test -p scoopc refactor_llvm_source_slice_classification`；`cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`；`cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P6-T03c：实现 refactor pure statement lowering，停止调用 legacy statement-level lowering

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.6.1-§5.6.2
  - `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`（只能作 primitive 抽取参考）
- 目标：
  - refactor backend 能独立 lower source-slice 中 effect-neutral statements。
  - 旧 `codegen_mir_statement` 不再作为通用 fallback。
- 必须实现的内容：
  1. 使用 `P6-T03a` 的 primitive lower local assignment、literal/const、tuple/scalar construction、field/member read-write、primitive arithmetic/compare/cast、top-level immutable/object refs。
  2. 对 closure/object/function value materialization 只使用已发布 callable carrier / source value ABI contract。
  3. 对 pure direct/runtime helper call 与 effectful/dynamic call 建立明确分界；不能把 effectful call 当普通 statement lower。
  4. 对 unsupported MIR statement fail fast，并指出缺少的 classification 或 value primitive。
  5. 删除或守卫 refactor body emitter 对 legacy statement-level lowering 的直接调用。
- 必须遵从的约束：
  - 禁止旧函数 ABI、旧 return lowering、旧 call dispatcher 进入 refactor statement lowering。
  - 禁止把 member read/write 回退到 HIR 或 field name guess。
- 验证：
  - `cargo test -p scoopc refactor_llvm_pure_statement_lowering`
  - `cargo test -p scoopc refactor_llvm_member_read_write_lowering`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
- 完成条件：
  - refactor body emitter 可独立 lower classified pure source slices。
  - legacy statement-level fallback 已移除或在 refactor path 上 fail fast。
- 依赖：P6-T03b
- 完成记录：
  - 2026-05-05：`crates/scoopc/src/llvm/codegen/effect_refactor/value.rs` 现在由 refactor value primitive 本地分派 classified pure source statements，不再委托 legacy/shared 的整条 MIR statement lowering；旧 `codegen_mir_effect_neutral_statement` 已删除。
  - 2026-05-05：source-slice classification 已把 `ClassCtor`、`MakeClosure` 与 pure `Call` 纳入 `effect-neutral-value`，由 refactor body emitter 显式 lower 或 fail fast；effectful/dynamic/virtual/interface/resume call 不会被当作普通 pure statement 静默 lower。
  - 2026-05-05：pure direct call lowering 现在通过 published refactor callable layout 调用 direct entry，按 source value ABI 打包实参，并从 returned `Step_F::Complete` 提取 payload；class/object/closure carrier materialization 继续通过已发布 callable carrier target 保持 refactor dynamic entry。
  - 2026-05-05：修正 body emitter 对 ABI visibility layout 的消费边界：按 callable `body_version_key` 取得 ABI step/frame layout，避免 `--opt-level 0` primary handoff 的局部 schema id 与 ABI visibility handoff 漂移时取错 completion binding。
  - 验证通过：`cargo test -p scoopc refactor_llvm_pure_statement_lowering`；`cargo test -p scoopc refactor_llvm_member_read_write_lowering`；`cargo test -p scoopc refactor_effect_lowered_source_slice_classification`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P6-T03d：闭合 refactor function ABI 与 entry shell lowering，包括 main wrapper

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §5.2, §5.6
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout,body}.rs`
  - `crates/scoopc/src/llvm/emit.rs`
- 目标：
  - direct entry、dynamic entry、resume method、surface resume、owner trampoline 与 main wrapper 都只由 refactor ABI query 驱动。
- 必须实现的内容：
  1. direct/dynamic entry 使用 callable version key 与 source value ABI，不依赖 `root_fqn -> unique shell`。
  2. resume method / surface resume / owner trampoline 消费 published continuation route、wrapper projection 与 resume payload binding。
  3. main wrapper 处理 `Step_F::Complete`、unhandled outward case、`Unit`/`Int`/其它返回类型诊断、以及 `Array<String>` argv ABI 或显式 fail-fast contract。
  4. dynamic entry 与 direct entry 的 args ABI 一致性由 ABI query 校验。
  5. 不允许 legacy function entry / old callable wrapper 混入 refactor callable body。
- 必须遵从的约束：
  - 禁止 main wrapper 对合法 outward case 直接 `unreachable`，除非 contract 明确证明不可达。
  - 禁止 refactor dynamic entry 指向 legacy callable ABI。
- 验证：
  - `cargo test -p scoopc refactor_llvm_function_abi`
  - `cargo test -p scoopc refactor_llvm_main_wrapper`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop -o /tmp/p6_refactor_main.ll`
- 完成条件：
  - refactor function shell 不再依赖 legacy function ABI。
  - main wrapper 对 refactor `Step_F` 协议有完整语义或显式诊断。
- 依赖：P6-T03c
- 完成记录：
  - 2026-05-05：refactor callable body emission 现在在定义 direct/dynamic entry 前校验 ABI query 发布的 entry pair contract，确保 direct/dynamic 的 invoke args tuple、参数 elision、参数个数与返回 `StepSchema` 一致，并且 callable body 继续按 `body_version_key` 选择 ABI layout。
  - 2026-05-05：refactor C `main` wrapper 改为只通过 published refactor direct entry 调用入口；零参入口不再用默认零值伪造 args payload，遇到非 elided args ABI 会 fail fast。`main(args: Array<String>)` 在 refactor argv tuple ABI 发布前会在 body emission 之前显式拒绝，避免借 legacy `scoop_entry_argv_array` 路径继续前进。
  - 2026-05-05：`main` wrapper 现在区分 `Step_F::Complete` 与 non-Complete `Step`：`Unit`/`Int` Complete 继续映射为进程 exit code；当入口 `Step` layout 发布了 outward cases 时，non-Complete 分支进入显式非零 exit path，而不是把合法 outward case 表示为 `unreachable`。仅当 ABI contract 证明入口 step 没有 outward cases 时，保留 unreachable 分支。
  - 2026-05-05：新增定向测试覆盖 refactor direct/dynamic entry shell、C main wrapper 调用 refactor direct entry、unhandled outward case 的显式 exit path，以及 Array<String> argv ABI 未发布时的 fail-fast 诊断。
  - 验证通过：`cargo test -p scoopc refactor_llvm_function_abi`；`cargo test -p scoopc refactor_llvm_main_wrapper`；`cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop -o /tmp/p6_refactor_main.ll`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P6-T03e：闭合 direct/dynamic/virtual/interface call lowering，不再回 legacy callable wrapper

- 参考：
  - [`TODO-P6-part1.md`](./TODO-P6-part1.md) `P6-T02d`, `P6-T02f`, `P6-T02g`
  - [`TODO-P6-part2.md`](./TODO-P6-part2.md) `P6-T02p`
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §5.6
- 目标：
  - refactor backend 对 known direct、closure/fun-value、virtual、interface、dynamic fallback 都使用 canonical `invoke(args_tuple) -> Step_F` contract。
- 必须实现的内容：
  1. Known instance call 用 published callable version selector 直接调用 refactor direct entry。
  2. Closure/fun-value/virtual/interface call 通过 published callable carrier target 和 canonical dynamic entry lower。
  3. Non-boundary dynamic call 与 boundary dynamic call 共用同一 call target query，不再走旧 callable wrapper。
  4. Call result 的 `Step_F` dispatch 与 boundary/complete contract 接通。
  5. 对缺失 carrier、ABI 漂移、callee version 歧义 fail fast。
- 必须遵从的约束：
  - 禁止在 body emitter 中按 `CallKind` 现场选择 legacy dispatch helper。
  - 禁止把 dynamic invoke 限缩成只支持当前 fixture 的 known direct call。
- 验证：
  - `cargo test -p scoopc refactor_llvm_call_lowering`
  - `cargo test -p scoopc refactor_llvm_dynamic_invoke_lowering`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
- 完成条件：
  - 所有 refactor call lowering 都由 published callable ABI/query 驱动。
  - refactor path 不再需要 legacy callable wrapper。
- 依赖：P6-T03d
- 完成记录：
  - 2026-05-05：refactor body lowering 现在对 `RefactorCallTargetQuery::KnownInstance` 统一调用 published direct entry；对 `RefactorCallTargetQuery::DynamicInvoke` 通过 published carrier source、`RefactorDynamicInvokeLayout` 与 canonical receiver/args ABI 发出 indirect `invoke(...) -> Step_F` 调用，不再把 dynamic boundary 作为 unsupported path。
  - 2026-05-05：source-slice 中的 non-boundary `Closure` / `FunValue` / `Virtual` / `Interface` call 现在按 `(owner_step_schema, site_id)` 回查 published dynamic-invoke layout，调用后要求 no-outward `Complete` 并写回目标 local；若返回 step 仍含 outward cases，会显式要求走 boundary lowering。
  - 2026-05-05：closure/vtable/itable carrier target shell 现在有 refactor wrapper body，把 receiver/closure-env 与 explicit args 组装成 callable direct-entry args tuple，再调用 published direct entry；virtual/interface carrier layout 同步发布 method slot / interface id，body emitter 不再按 legacy dispatch helper 现场恢复 slot。
  - 2026-05-05：更新 `effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`，让 `main` 真实触发 pure source-slice virtual dynamic call，并锁定 emitted LLVM 中的 `refactor_dynamic_call_step`。
  - 验证通过：`cargo test -p scoopc refactor_llvm_call_lowering`；`cargo test -p scoopc refactor_llvm_dynamic_invoke_lowering`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P6-T03f：闭合 boundary lowering，覆盖 Call / Perform / Resume / runtime-error / nested-handle outward

- 参考：
  - [`TODO-P6-part2.md`](./TODO-P6-part2.md) `P6-T02o`, `P6-T02q`, `P6-T02qd`, `P6-T02qg`, `P6-T02qh`
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.5, §5.6
- 目标：
  - boundary lowering 只消费 published operand/source/dispatch/resume payload/completion payload contract。
- 必须实现的内容：
  1. `Perform` 构造 outward `Step_F` case，捕获 continuation，写入 payload。
  2. `Call` 调用 refactor callee entry，对 callee `Step_F` 做 complete/outward dispatch。
  3. `Resume` 调用 published surface resume route，对 returned owner/wrapper `Step_F` 做 complete/outward dispatch。
  4. ordinary runtime error outward lower 成普通 `Step_F` case，不走 hidden trap/outcome。
  5. nested-handle outward boundary 进入统一 outward `Step_F` 模型，self-contained handle 不向外扩散。
  6. boundary result / resume payload / completion payload 写入 published home slot。
- 必须遵从的约束：
  - 禁止 boundary lowering 回 raw MIR statement/terminator 恢复 callee、args、payload、receiver 或 result target。
  - 禁止把 `RuntimeError` / `Handle` boundary 当作 unsupported primary boundary 长期拒绝。
- 验证：
  - `cargo test -p scoopc refactor_llvm_boundary_lowering`
  - `cargo test -p scoopc refactor_llvm_runtime_error_case`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
- 完成条件：
  - 所有 boundary categories 已在 refactor LLVM CFG 中闭合。
  - backend 不再通过 raw MIR shape 恢复 boundary semantics。
- 依赖：P6-T03e
- 完成记录：
  - 2026-05-05：refactor LLVM body lowering 现在显式覆盖 `Call` / `Perform` / `Resume` / `RuntimeError` / `Handle` boundary lowering 分支；`RuntimeError` boundary 通过普通 `Step_F` outward case 传播 `RuntimeError.ContinuationAlreadyResumed` payload，`Handle` boundary 通过已发布 outward emission 进入统一 outward `Step_F` helper，不再把合法 primary boundary 长期拒绝为非 `Call/Perform/Resume`。
  - 2026-05-05：补齐当前 P6-T03f run-pass 验证的直接前置缺口：refactor value primitive 会跳过未使用的 sysroot internal print callee ref，直接把 `__scoop_print[_ln]_string(String)` lower 到 runtime print ABI；primitive/String `ToString.toString` interface call 在缺少 dynamic-invoke handoff 时走明确的 builtin runtime primitive，用户类型仍要求 published dynamic-invoke contract。
  - 2026-05-05：新增 `refactor_llvm_boundary_lowering` 与 `refactor_llvm_runtime_error_case` 定向单测，锁定 boundary category 覆盖和 runtime-error ordinary step payload 路径。
  - 验证通过：`cargo test -p scoopc refactor_llvm_boundary_lowering`；`cargo test -p scoopc refactor_llvm_runtime_error_case`；`cargo test -p scoopc refactor_llvm_value_primitive`；`cargo test -p scoopc refactor_llvm_dynamic_invoke_lowering`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`；`cargo clippy --all-targets -- -D warnings`。
  - `PLAN.md` 无需改动。

## [DONE] P6-T03g：闭合 HandleDispatch protocol，覆盖 body / arm / finally / exit / pending completion transport

- 参考：
  - [`TODO-P6-part1.md`](./TODO-P6-part1.md) `P6-T02j`, `P6-T02k`, `P6-T02l`
  - [`TODO-P6-part2.md`](./TODO-P6-part2.md) `P6-T02qb`
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.5.6, §5.6
- 目标：
  - `HandleDispatch` 不再只是 branch 到 body state，而是完整 lower published handle protocol。
- 必须实现的内容：
  1. body/arm/finally/exit state-region routing 消费 published contract。
  2. handled case payload binder 与 continuation binder 写入 published local/frame slot。
  3. body complete、arm complete、finally complete、continue-to-exit、return-from-function、propagate-outward 都通过 published completion-state protocol lower。
  4. pending payload transport 使用 published carrier layout，不现场发明 boxing/projection。
  5. cleanup/finally 与 dropped continuation 的交互遵守 P5 state graph。
- 必须遵从的约束：
  - 禁止借用 legacy handle magic state tag 或 backend-private completion constants。
  - 禁止回 canonical MIR/HIR 恢复 arm binder、finally path 或 handle region。
- 验证：
  - `cargo test -p scoopc refactor_llvm_handle_dispatch_lowering`
  - `cargo test -p scoopc refactor_llvm_handle_pending_completion`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/handle_finally_boundary.scoop`
- 完成条件：
  - Handle protocol 的所有 published path 都能 lower 到 LLVM CFG。
  - backend 不再重建 handle 局部状态机。
- 依赖：P6-T03f
- 完成记录：
  - 2026-05-05：`HandleDispatch` body/arm/finally/exit routing 现在由 published `RefactorHandleDispatchLayout` 驱动；body/arm completion、boundary case consumption、pending completion tag 与 pending outward payload transport 均在 refactor LLVM body emitter 中显式 lower。
  - 2026-05-05：late-lowered handoff 新增 body completion payload source，避免 surface `resume` 在 body completion 时提前进入 `finally`；wrapper completion 现在使用 published body/arm payload source 投影，`finally` 在 immediate resume 路径只执行一次。
  - 2026-05-05：frame lifting 为 handle-only callable 保留 system frame slots，并为 pending outward payload 发布 typed `HandlePendingPayload` slot；ABI query 校验/发布 pending payload transport field、completion tag field 与 arm binder layout。
  - 2026-05-05：新增 `tests/fixtures/run-pass/handle_finally_boundary.scoop`，覆盖 `resume -> body tail -> finally -> exit` 的 refactor run-pass 路径；新增 `refactor_llvm_handle_dispatch_lowering` / `refactor_llvm_handle_pending_completion` 定向测试入口。
  - 验证通过：`cargo test -p scoopc refactor_llvm_handle_dispatch_lowering`；`cargo test -p scoopc refactor_llvm_handle_pending_completion`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/handle_finally_boundary.scoop`；额外回归 `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_finally_normal.scoop`；`cargo clippy --all-targets -- -D warnings`。
  - `PLAN.md` 无需改动。

## [DONE] P6-T03h：闭合 continuation protocol，覆盖 one-shot、double resume、wrapper projection、drop/unwind/abandon

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.2-§5.3.7, §5.5, §5.6
  - [`TODO-P6-part2.md`](./TODO-P6-part2.md) `P6-T02m`, `P6-T02q`, `P6-T02qc`, `P6-T02qd`, `P6-T02qh`
- 目标：
  - continuation 对象与 resume protocol 不再包含 placeholder `unreachable` 或 owner/wrapper 私有猜测。
- 必须实现的内容：
  1. continuation allocation、captured frame、resume state、one-shot flag 按 published layout lower。
  2. resume entry 从 continuation 恢复 frame/local，注入 resume payload，跳转到 published resume state。
  3. double resume 产生 ordinary runtime error outward case，而不是 trap-only 或 `unreachable`。
  4. shared wrapper projection 覆盖 complete 与 outward case，并消费 published wrapper payload source。
  5. `ResumeUnwind` / `Abandon` / dropped continuation 语义显式 lower 或 fail fast 到 contract-defined path。
- 必须遵从的约束：
  - 禁止 continuation runtime behavior 依赖 legacy handler stack。
  - 禁止用 `unreachable` 代表普通语言级错误或 dropped continuation 语义。
- 验证：
  - `cargo test -p scoopc refactor_llvm_continuation_protocol`
  - `cargo test -p scoopc refactor_llvm_double_resume_runtime_error`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_escape_continuation_resume_later_exit.scoop`
- 完成条件：
  - continuation/resume semantics 已在 refactor LLVM path 上闭合。
  - double resume、drop/unwind/abandon 不再是 placeholder。
- 依赖：P6-T03g
- 完成记录：
  - 2026-05-05：same-schema resume-boundary surface route 现在也会在 owner `StepSchema` 与 wrapper `out_step_schema` 不同时发布 wrapper projection；ABI materialization 允许多个 resume site 共享同一 projection shape，并继续对真正漂移 fail fast。
  - 2026-05-05：refactor LLVM resume entry 现在按 published continuation object layout 恢复 frame/local、检查并置位 one-shot flag；double resume 不再 `unreachable`，而是构造普通 `RuntimeError.ContinuationAlreadyResumed` outward `Step` 并走既有 wrapper projection / boundary dispatch。
  - 2026-05-05：resume-entry handle completion 现在按 published handle completion payload contract 返回 caller，避免 escaped continuation resume 后重放已完成的外层 handle continuation；同一 handler arm 内已发布为 completion payload 的 resume 结果会完成 handle，而不是继续执行后续重复 resume。
  - 2026-05-05：`ResumeUnwind` / `Abandon` 不再合并到裸 placeholder 分支；`ResumeUnwind` 对缺失 unwind payload/cleanup continuation contract fail fast，`Abandon` 只允许 contract-defined drop state 终止，防止普通 CFG 直接触达 dropped-continuation path。
  - 验证通过：`cargo test -p scoopc refactor_llvm_continuation_protocol`；`cargo test -p scoopc refactor_llvm_double_resume_runtime_error`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_escape_continuation_resume_later_exit.scoop`；相邻回归 `cargo test -p scoopc refactor_effect_lowered_surface_resume_wrapper_completion`；`cargo test -p scoopc refactor_llvm_surface_resume_wrapper_completion`；`cargo test -p scoopc refactor_llvm_handle_dispatch_lowering`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/handle_finally_boundary.scoop`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P6-T03i：闭合 runtime error、diagnostics 与 body verifier，冻结 clean body lowering 完成条件

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.9, §5.6
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{body,layout}.rs`
- 目标：
  - runtime error 与 unsupported path 都有明确语义或明确诊断。
  - P6-T03 系列完成后，body lowering 可以进入 review。
- 必须实现的内容：
  1. `LocalRuntimeError` materialize payload，不再传 null 占位。
  2. ordinary runtime error 作为 `Step_F` case 传播；fatal runtime entry 只用于 contract 明确的 terminal path。
  3. main unhandled outward case 进入明确 runtime/diagnostic path，不长期 `unreachable`。
  4. 建立 body verifier，检查 state block、boundary owner/resume、frame slot、payload source、source-slice classification、ABI layout 是否完整。
  5. 搜索并清理 refactor backend 中剩余的 silent skip、默认值、placeholder unreachable、legacy semantic fallback。
- 必须遵从的约束：
  - 禁止 hidden trap/outcome channel。
  - 禁止 diagnostics 只在测试中绕过真实 refactor CLI path。
- 验证：
  - `cargo test -p scoopc refactor_llvm_body_verifier`
  - `cargo test -p scoopc refactor_llvm_runtime_error_lowering`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `rg "codegen_mir_statement|build_unreachable\(\).*double|尚未支持 dynamic invoke|primary boundary.*不是 Call/Perform/Resume|null_payload" crates/scoopc/src/llvm/codegen/effect_refactor`
- 完成条件：
  - refactor LLVM body lowering 对 function protocol、call、boundary、handle、continuation、runtime error 已闭合。
  - 不再有已知合法语义路径靠 placeholder `unreachable`、默认值或 legacy fallback 通过。
- 依赖：P6-T03h
- 完成记录：
  - 2026-05-05：refactor LLVM body emitter 现在在 callable entry 初始化时运行 body verifier，检查 state graph/state block、boundary owner/resume、source-slice classification、frame/resume/completion payload binding、boundary operand/layout 与 local runtime-error contract 的一致性。
  - 2026-05-05：`LocalRuntimeError` 不再用空 payload 调用 fatal runtime；call-boundary 本地消费 runtime-error case 会从 callee `Step` 提取实际 payload，按 published `RefactorLocalRuntimeErrorContract` 调用 `scoop_runtime_error_fatal`，普通 runtime-error boundary 仍作为 `Step_F` case 传播。
  - 2026-05-05：补齐直接阻塞验证的 body lowering 缺口：handle arm boundary completion 回到 handle return state 前会把 boundary result 复制到 published return payload local；double resume runtime-error 选择也能消费 `Step` schema 中已发布但没有 paired runtime-error boundary 的 runtime-error case。
  - 2026-05-05：新增 `refactor_llvm_body_verifier` 与 `refactor_llvm_runtime_error_lowering` 定向测试，并保留搜索守卫，确认 refactor backend 中没有 `null_payload`、legacy statement fallback 或已知 placeholder 文本。
  - 验证通过：`cargo test -p scoopc refactor_llvm_body_verifier`；`cargo test -p scoopc refactor_llvm_runtime_error_lowering`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`；`rg "codegen_mir_statement|build_unreachable\(\).*double|尚未支持 dynamic invoke|primary boundary.*不是 Call/Perform/Resume|null_payload" crates/scoopc/src/llvm/codegen/effect_refactor`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P6-T03R：Review clean LLVM body lowering，确认 backend 拥有 whole function protocol 且不再胶合 legacy codegen

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.6
  - [`PLAN.md`](./PLAN.md) §2/P6
- 重点：
  - refactor backend 是否拥有 entry/return ABI、state CFG、boundary、handle、continuation、runtime error、diagnostics。
  - legacy codegen 是否只剩 effect-neutral primitive 复用。
  - source-slice statements 是否都由 classification 驱动，没有 silent skip。
  - dynamic/virtual/interface call 是否都走 canonical refactor invoke contract。
  - runtime error、double resume、drop/unwind/abandon 是否有真实语义而非 placeholder。
- 验证：
  - 重新运行 `P6-T03a` ~ `P6-T03i` 的全部测试与命令。
  - `rg "codegen_mir_statement|build_unified_lowering_contract|effect_analysis_ctx|effect_op_call_sites|hir::HandleExpr|scoop_effect_handler_stack|scoop_effect_outcome" crates/scoopc/src/llvm/codegen/effect_refactor crates/scoopc/src/effect_refactor_pipeline`
- 完成条件：
  - review 能明确说明 P6 body lowering 已符合 clean refactor backend 边界。
  - 可进入 `P6-T04`。
- 依赖：P6-T03i
- 完成记录：
  - 2026-05-05：完成 clean LLVM body lowering review。确认 refactor body emitter 的 entry/return ABI、state CFG、boundary、HandleDispatch、continuation、runtime-error 与 diagnostics 均由 P5/P6 published contract 和 ABI query 驱动；`source_statement_classification` 仍是 source-slice statement lowering 的入口，没有恢复 private skip heuristic。
  - 2026-05-05：review 中修复阻断项：`ResumeUnwind` / `Abandon` 重新拆成显式 verifier/lowering path，`Abandon` 只允许 published drop state，cleanup `ResumeUnwind` 只允许 contract-defined cleanup terminal；suspend state 多 primary boundary 会 fail fast；direct entry args ABI 漂移和 handle arm non-elided payload 缺失不再用默认值或静默跳过。
  - 2026-05-05：review 中修复 legacy 胶合风险：refactor closure carrier materialization 在 callable-carrier contract 启用时不再为了 fallback 定义 legacy lambda body；refactor class constructor lowering 不再调用 legacy class ctor invoke/effect propagation helper，当前支持 primary ctor 参数属性写入，非平凡 ctor body/delegation/init step 继续 fail fast 等待 published ctor ABI。
  - 2026-05-05：review 中修复 ordinary runtime-error payload materialization，使 RuntimeError payload 按 ABI/source payload type 直接构造，不再要求 HIR compat `TypeStore` 中出现 `scoop.core.RuntimeError` 类型 id。
  - 2026-05-05：指定 grep 仅剩允许命中：`effect_refactor_pipeline/hir_stage.rs` 中 typed-HIR contract publication 使用 `effect_op_call_sites` / `hir::HandleExpr`，以及 `effect_refactor/layout.rs` 中 `#[cfg(test)]` ABI materialization scaffolding 传入 `effect_op_call_sites`；refactor body emitter 中没有 legacy statement/function fallback、handler-stack/outcome 依赖。
  - 验证通过：`cargo test -p scoopc refactor_llvm_clean_backend_boundary`；`cargo test -p scoopc refactor_llvm_value_primitive`；`cargo test -p scoopc refactor_effect_lowered_source_slice_classification`；`cargo test -p scoopc refactor_llvm_source_slice_classification`；`cargo test -p scoopc refactor_llvm_pure_statement_lowering`；`cargo test -p scoopc refactor_llvm_member_read_write_lowering`；`cargo test -p scoopc refactor_llvm_function_abi`；`cargo test -p scoopc refactor_llvm_main_wrapper`；`cargo test -p scoopc refactor_llvm_call_lowering`；`cargo test -p scoopc refactor_llvm_dynamic_invoke_lowering`；`cargo test -p scoopc refactor_llvm_boundary_lowering`；`cargo test -p scoopc refactor_llvm_runtime_error_case`；`cargo test -p scoopc refactor_llvm_handle_dispatch_lowering`；`cargo test -p scoopc refactor_llvm_handle_pending_completion`；`cargo test -p scoopc refactor_llvm_continuation_protocol`；`cargo test -p scoopc refactor_llvm_double_resume_runtime_error`；`cargo test -p scoopc refactor_llvm_body_verifier`；`cargo test -p scoopc refactor_llvm_runtime_error_lowering`；`cargo test -p scoopc refactor_effect_lowered_surface_resume_wrapper_completion`；`cargo test -p scoopc refactor_llvm_surface_resume_wrapper_completion`。
  - 验证通过：`cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`；`cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`；`cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop -o /tmp/p6_refactor_main.ll`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/handle_finally_boundary.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_finally_normal.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_escape_continuation_resume_later_exit.scoop`；required `rg "codegen_mir_statement|build_unified_lowering_contract|effect_analysis_ctx|effect_op_call_sites|hir::HandleExpr|scoop_effect_handler_stack|scoop_effect_outcome" crates/scoopc/src/llvm/codegen/effect_refactor crates/scoopc/src/effect_refactor_pipeline`；`cargo clippy --all-targets -- -D warnings`。
  - 额外非必需检查：`cargo test -p scoopc refactor_llvm_` 仍会触发不属于 P6-T03a~P6-T03i 指定过滤器的既有 layout/stage tests，其中若干 synthetic ABI tests 缺少 call-boundary continuation composition 的 surface-resume ABI scaffolding；本轮未把这些历史 broad-filter failures 作为 P6-T03R 完成条件。

## P6-T04：接通 GC roots / stackmaps / runtime 语义，并锁定 dropped continuation、runtime error 与 Managed ABI 边界

- 来源：从 [`TODO-P6-part2.md`](./TODO-P6-part2.md) 迁移并按 clean backend 边界更新。
- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.7, §5.3.8, §5.3.9, §5.5.6, §5.6, §8
  - `crates/scoopc/src/llvm/pipeline.rs`
  - `crates/scoopc/src/llvm/stackmap.rs`
  - `crates/scoopc/src/llvm/codegen/gc.rs`
  - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs`
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
- 目标：
  - 让 clean refactor LLVM path 在 GC/runtime/stackmap 层面对 `Step` / frame / continuation / runtime error 端到端闭合。
- 必须实现的内容：
  1. frame object、continuation object、`Step_F` payload 接入 GC object header、trace/scan metadata、allocation path 与 root model。
  2. 跨 safepoint/statepoint 活跃的 frame slots、continuation captures、resume carriers、payload refs 可被 moving GC 正确追踪和写回。
  3. ordinary runtime error 保持普通 `Step_F` case 语义。
  4. dropped continuation 只表示剩余语言级计算被放弃，不触发 pending finally/cleanup 继续执行。
  5. Managed ABI / extern 边界不承载 `Step_F` / continuation / resume interface；不支持场景保持明确诊断。
  6. refactor emitted IR 不以 legacy handler-stack / outcome runtime calls 作为 correctness 前提。
- 必须遵从的约束：
  - 禁止 raw `malloc` 作为最终 continuation/frame allocation contract。
  - 禁止把 cleanup hook 当成 dropped continuation 语义路径。
  - 禁止因为不是 full regression 就跳过 moving GC / stackmap / roots 验证。
- 验证：
  - `cargo test -p scoopc refactor_llvm_gc_roots`
  - `cargo test -p scoopc refactor_llvm_stackmap`
  - `cargo test -p scoopc refactor_llvm_dropped_continuation`
  - `cargo test -p scoopc refactor_llvm_managed_abi_boundary`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_no_legacy_handler_stack_calls.scoop`
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/runtime_gc/effect_outer_mutable_state_body_writeback_basic.scoop`
- 完成条件：
  - refactor LLVM path 已在 GC/runtime/stackmap 层面对 clean body lowering 输出闭合。
  - emitted IR 不再以 legacy handler-stack/outcome runtime contract 为 correctness 前提。
- 依赖：P6-T03R
- 完成记录：
  - （执行时填写）

## P6-T04R：Review GC/runtime 集成，确认 clean refactor path 没有 legacy runtime 语义依赖

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.7, §5.3.8, §5.3.9, §5.6, §8
  - [`PLAN.md`](./PLAN.md) §2/P6
- 重点：
  - GC roots / stackmap / object reachability 是否正确。
  - dropped continuation 是否只表现为剩余计算被放弃。
  - runtime error 是否仍是普通 effect 分支。
  - Managed ABI / extern 是否仍保持 pure-only 边界。
  - 是否已经摆脱 legacy handler-stack / outcome runtime contract。
- 验证：
  - 重新运行 `P6-T04` 的全部测试与命令。
  - `rg "scoop_effect_handler_stack|scoop_effect_outcome|LegacyEffectBoundary|cleanup hook|Managed ABI|extern" crates/scoopc/src/llvm/codegen/effect_refactor crates/scoopc/src/effect_refactor_pipeline crates/scoopc/src/llvm/codegen/effect`
- 完成条件：
  - review 能明确说明 GC/runtime/FFI 语义边界已经按 clean refactor 设计锁定。
  - 可进入 `P6-T05`。
- 依赖：P6-T04
- 完成记录：
  - （执行时填写）

## P6-T05：建立 refactor LLVM 定向 build/run-pass/runtime_gc 验证矩阵，并冻结 P6 -> P7 handoff contract

- 来源：从 [`TODO-P6-part2.md`](./TODO-P6-part2.md) 迁移并按 clean backend 边界更新。
- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6，§2/P7
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §4.16, §5.2, §5.3.7-§5.3.9, §5.5, §5.6, §8
  - `crates/scoop/src/commands/build.rs`
  - `crates/scoop/src/commands/run.rs`
  - `crates/scoop/src/fixtures/mod.rs`
- 目标：
  - 用现有 build / run-pass / runtime_gc fixture phase 建立可重复的 refactor LLVM 验证矩阵。
  - 明确 P7 只做 selector flip + full regression，不再重做 LLVM effect backend 设计。
- 必须实现的内容：
  1. build fixtures 覆盖 `NoOutward`、`SingleCase`、`CanonicalFull`、direct perform/handle/resume、dynamic callable fallback、one-shot/runtime error、Unit 零载荷、无 legacy handler-stack/outcome calls。
  2. run-pass fixtures 覆盖 direct/indirect effect call、continuation capture/resume、dynamic invoke、dropped continuation、nested handle outward/self-contained。
  3. runtime_gc fixtures 覆盖 frame/continuation roots、delayed/cross-thread resume、effect body writeback、payload refs。
  4. build/run/fixture/emit-llvm/emit-obj/emit-asm 都共享同一 refactor LLVM stage/helper。
  5. 至少锁定一组 `O0` 与一组较高优化级别样本，证明差异只来自 impl plan / P5 post-opt / LLVM pass pipeline。
  6. 文档或代码注释固化 P6 -> P7 handoff contract。
- 必须遵从的约束：
  - 禁止新增只供测试绕过真实 CLI/stage 的语义入口。
  - 禁止在 P6-T05 才补高层 effect lowering 逻辑。
  - 禁止提前执行 P7 selector flip 或 full regression。
- 验证：
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_no_legacy_handler_stack_calls.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/runtime_gc/effect_outer_mutable_state_body_writeback_basic.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop -o /tmp/p6_refactor_resume.ll`
  - `cargo run -p scoop -- --effect-pipeline refactor run tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
- 完成条件：
  - 仓库中已有可重复运行的 refactor LLVM 定向验证矩阵。
  - P6 -> P7 handoff contract 已通过代码、测试或文档锁定。
- 依赖：P6-T04R
- 完成记录：
  - （执行时填写）

## P6-T05R：Review P6 阶段退出条件，确认 P7 只需切主线并执行 full regression

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6，§2/P7，§3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §4.16, §5.2, §5.3.7-§5.3.9, §5.5, §5.6, §8
- 重点：
  - refactor LLVM codegen stage 是否已独立存在，并成为 build/run/fixture 的共同入口。
  - body lowering 是否符合 clean backend 边界，不再胶合 legacy statement/function codegen。
  - `Step_F` / frame / continuation object / resume surface / wrapper projection / call invoke / handle / runtime error / GC roots 是否闭合。
  - 定向 build/run-pass/runtime_gc 矩阵是否已建立。
  - P7 是否可以只做 selector flip + full regression。
- 验证：
  - 重新运行本文件 `P6-T02qh` ~ `P6-T05` 的全部定向测试与命令。
  - 不执行 `cargo test --all` 或全量 `cargo run -p scoop -- test`；这些 broad regression 留到 P7。
- 完成条件：
  - review 能明确说明 P6 已完成 clean refactor LLVM codegen 对接。
  - P7 可以在不重新讨论 LLVM effect backend 架构、ABI、GC/runtime 或 legacy fallback 边界的前提下进入默认主线切换。
- 依赖：P6-T05
- 完成记录：
  - （执行时填写）
