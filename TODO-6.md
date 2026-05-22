# TODO-6：Global init + LLVM backend cleanup + final verification

> 生成时间：2026-05-21
> 细化时间：2026-05-22
> 计划基线：[`PLAN.md`](./PLAN.md) §4/P6-P8
> 索引：[`TODO.md`](./TODO.md)
> 当前状态：`P7-T02` 已完成；下一步执行 `P7-T02-a`。

## 范围

- 闭合 global object、top-level `val`、annotated top-level `var` 的初始化根分类。
- 分离 object once 与 top-level eager init 语义。
- 建立 per-cone init routine 与 final entry init order contract。
- 让 `@Global` / `@ThreadLocal` storage policy 全链路闭合。
- 清理 LLVM backend，使其只依赖 `LIR + LIR facts + base context`。
- 做最终全仓验证，为未来 C backend 固定干净输入边界。

## 进入门禁

- P4/P5 已完成：effect facts stage 不修改 MIR 输出本体，`EffectFactsStageOutput = { effect_facts }`，`LirStageOutput = { lir, lir_facts }`，且 LIR opt family 只消费 LIR-owned 输入。
- `scoopc_lir_facts` 已发布 P5-owned backend-neutral callable ABI、dynamic invoke、dispatch owner/slot、continuation/resume publication 与 LIR opt metadata；TODO-6 不应重新让 LLVM 从 HIR/raw MIR/effect facts 推导这些合同。
- TODO-6 从剩余边界开始：global init/storage/entry init order、LLVM HIR scaffold、`LlvmCodegenStageOutput` / `StageEmitInput` P5 wrapper handoff、crate-private MIR pass-view residual、LLVM physical ABI/layout、backend reachability、多 `TypeStore` 桥接和最终全仓验证。

## 细化原则

- 每个实现小阶段后必须插入独立 review 任务。
- P6 global init 与 P7 backend cleanup 分开推进，禁止在 codegen 里补前端语义。
- 如果 LLVM backend 仍需要 HIR/raw MIR/effect facts，必须先补齐 LIR/LIR facts 或 base context owner，不能在 backend 增加新的回看路径。
- 最终验收必须搜索确认：无旧 comptime surface、无 HIR/codegen 层去虚化、无 stage output 嵌套上游整包。

## TODO-6-INIT 审计基线

### Global init / storage 当前入口

| 层级 | 当前入口 | 现状与分歧点 |
| --- | --- | --- |
| HIR side table | `crates/scoopc/src/hir/mod.rs` 的 `ExternGlobal`、`TopLevelVar`、`TopLevelImmutableValue`、`ObjectInit` | 实际 initializer body 仍在 `LoweredHir` side table；P7 前 LLVM 仍读取这些 scaffold，不能把它们当长期 facts。 |
| HIR lowering | `crates/scoopc/src/hir/lower/main/impl_lowering.rs` | top-level `val`、top-level `var`、`@Extern` 与 object init step 在这里分类；`@Global` / `@ThreadLocal` 通过 FQN 解析成 `TopLevelVarStorage`。 |
| HIR/typecheck gate | `crates/scoopc/src/typecheck/annotations.rs` | `@Extern` 顶层变量禁止 initializer；非 extern top-level `var` 必须显式 `@Global` / `@ThreadLocal` 且满足当前 GC-free gate。 |
| `hir_facts` | `crates/scoopc_hir_facts/src/globals.rs`、`crates/scoopc_hir_facts/src/source_sites.rs`、`crates/scoopc/src/pipeline/hir_stage.rs` | 已表达 `TopLevelVal` / `TopLevelVar` / `ObjectSingleton`、storage policy、monomorphic gate、top-level init root contract 与 dependency roots；不承载 initializer body。 |
| MIR / `mir_facts` | `crates/scoopc/src/mir/lower/mir_lowering_facts.rs`、`crates/scoopc/src/mir/lower/entry.rs`、`crates/scoopc/src/mir/mod.rs`、`crates/scoopc_mir_facts/src/roots.rs`、`crates/scoopc/src/pipeline/mir_stage.rs` | 已有 `InitializerRoot` / `ExternGlobalRoot` inventory、MIR `TopLevelRef` / `StoreTopLevelVar`；`mir_facts` 目前只发布 dependency count 等摘要，dependency 身份仍主要在 MIR item。 |
| LIR / `lir_facts` | `crates/scoopc_lir_facts/src/lib.rs`、`crates/scoopc/src/pipeline/lir_facts_builder.rs`、`crates/scoopc/src/pipeline/effect_lowering_stage.rs` | `LirFacts` 目前覆盖 callable/effect-control/dispatch/resume/opt metadata，尚未发布 global init/storage/per-cone/final-entry facts；`LirStageContext` 仍保留 LLVM residual pass view。 |
| LLVM globals | `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/main/globals.rs`、`crates/scoopc/src/llvm/codegen/main/immut_value.rs`、`crates/scoopc/src/llvm/codegen/object_init.rs`、`crates/scoopc/src/llvm/codegen/main/cone_init.rs`、`crates/scoopc/src/llvm/emit.rs` | LLVM 仍读取 HIR side tables；top-level `val` 走 lazy once first-access；object once 与 top-level `val` 共享 runtime once；per-cone init routine 当前基本为空壳，只在 final entry 中被调用。 |

### Global init / storage 语义分歧点

1. top-level `val` 仍是 lazy first-access，目标语义要求所有 linked cones 的 top-level `val` 在用户 `main` 前 eager 初始化。
2. object once 与 top-level eager init 仍共享旧 once/runtime helper 路径，目标语义要求 once 只服务 object singleton。
3. per-cone init routine 尚未按 LIR facts 执行本 cone roots；final entry 虽有调用位置，但没有闭合真实 init order。
4. `@Global` / `@ThreadLocal` 的 HIR/MIR facts 已存在，但 LLVM 仍自己决定物理 storage 与初始化方式；非 entry thread TLS 初始化策略还需要显式收口。
5. HIR/MIR 已记录 dependency roots，但 backend 当前不按 dependency contract 决定 init order，且 `mir_facts` 对 dependency 身份的发布不够直接。
6. top-level `var` initializer 当前偏向 LLVM const/static initializer 路径，非 const runtime initializer 与 eager init routine 的关系未闭合。
7. `LirFacts` 尚无 global init/storage contract，导致 LLVM 无法只凭 `LIR + lir_facts + base context` 完成 global init。
8. `@Extern` global root 在 source-site/MIR root 层有合同，但 LLVM 仍保留 HIR `extern_globals` scaffold 读取。

### LLVM backend 当前上游输入残余

| 位置 | 当前读取 | TODO-6 归属 |
| --- | --- | --- |
| `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` | `LlvmCodegenStageOutput` 保存 `LoweredHir` scaffold、`HirFacts`、primary/ABI visibility `EffectLoweredStageOutput`；stage 会从 `CodegenLoweringOutput` 重建 HIR/MIR/P4/P5 handoff。 | P7 删除 P5 wrapper handoff 与 HIR scaffold 长期输出。 |
| `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` | `precheck_invalid_integer_literals` 直接遍历 HIR item/fun/expr/pat 与 `TypeStore`。 | P7 把 backend 前置诊断迁到 HIR 屏障或 LIR/base context，不保留 backend HIR traversal。 |
| `crates/scoopc/src/llvm/emit.rs` | `StageEmitInput` 接收 HIR scaffold 与两份 `EffectLoweredStageOutput`；`LoweredCodegenEntry` 混合 LIR、LIR facts、HIR、pass view、TypeStore。 | P7 收窄 LLVM 入口输入。 |
| `crates/scoopc/src/llvm/emit.rs` | module 构建读取 HIR root/fun index、global side tables、entry main HIR body。 | P6/P7 迁移 global/entry 合同到 LIR facts 与 base context。 |
| `crates/scoopc/src/llvm/reachability.rs` | reachability 同时读取 HIR indexes/body、`MaterializedMirPassView`、raw MIR bodies/calls，并保留 backend devirtualization 推断。 | P7 改为 LIR/LIR facts reachability，删除 codegen 层 devirtualization。 |
| `crates/scoopc/src/llvm/codegen/mod.rs` | `CompilationUnitCodegenCx` 长期持有 HIR indexes、HIR facts、`MaterializedMirPassView`、LIR program、LIR facts、`TypeStore`。 | P7 收口为 backend-private LIR codegen context。 |
| `crates/scoopc/src/llvm/codegen/main/*.rs` | function/expr/stmt/class ctor fallback 直接 lower HIR。 | P7 删除 HIR fallback body emission。 |
| `crates/scoopc/src/llvm/codegen/mir_body/*.rs` | raw MIR body lowering 直接读取 MIR `Body`、`Statement`、`Rvalue`、`CallKind`、MIR `TypeStore`，并用 LIR signature 补丁。 | P7 删除 crate-private MIR pass-view body emission residual。 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/*.rs` | 以 LIR state graph 为主，但仍从 pass view 拉 MIR function/body/local/type 信息。 | P7 让 effect-lowered body emission 只依赖 LIR body/source-slice contract 与 base type context。 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/layout/*.rs` | physical ABI layout 主要消费 `LateLoweredProgram + LirFacts + TypeStore`，但 carrier/ABI/type layout 仍读取 HIR vtable/itable/class init/enum layout 与多 `TypeStore` bridge。 | P7 保留 backend-private physical layout，删除 HIR/effect/raw MIR facts 输入与多 TypeStore 模糊桥接。 |

## [DONE] TODO-6-INIT：初始化并细化本任务包

- 目标：
  - 分析 `PLAN.md` §4/P6-P8、`PIPELINE_REFACTOR.md` 和当前 global init、storage policy、LLVM backend 输入依赖的真实边界；
  - 生成本任务包的详细任务列表，覆盖 global init model、LLVM backend cleanup 和 final verification；
  - 更新 `TODO.md` 的具体任务索引，用新生成的任务扩展 `TODO-6-INIT` 所在索引行。
- 必须实现的内容：
  1. 列出当前 object once、top-level `val`、top-level `var`、`@Global`、`@ThreadLocal` 的 HIR/MIR/LIR/codegen 入口和语义分歧点。
  2. 列出 LLVM backend 当前直接读取 HIR、raw MIR、effect facts 或上游整包 stage output 的位置。
  3. 把 P6、P7、P8 拆成数量适中的实现小阶段，每个阶段必须有明确目标、修改范围、验证命令和完成条件。
  4. 在每个实现小阶段后插入独立 review 任务，review 任务必须复审前一阶段是否满足 global init contract 或 backend 输入边界约束。
  5. 同步更新 `TODO.md` 中的具体任务索引，确保任务 ID、状态和顺序与本文件一致。
- 完成条件：
  - `TODO-6.md` 不再只是范围说明，而是包含可直接执行的详细任务列表；
  - `TODO.md` 的具体任务索引已经同步反映 `TODO-6.md` 的新任务和 `[TODO]` 状态；
  - 本任务完成记录说明 P6/P7/P8 的拆分依据、最终验收命令和未来 C backend 输入边界风险。
- 完成记录：
  - 拆分依据：P6 先闭合 global init/storage/entry order，因为 P7 要删除 LLVM HIR scaffold 必须先有 LIR/LIR facts 承载这些合同；P7 再按入口/global、reachability/body、stage wrapper、physical ABI/TypeStore residual 收口 LLVM；P8 最后做全仓 residual 搜索、文档同步和完整验证。
  - 审计结论：当前 HIR/MIR facts 已有 root 分类、storage policy 与部分 dependency 合同，但 LIR facts 尚无 global init/storage surface；LLVM 仍从 HIR side tables、raw MIR pass view、HIR facts 与多 TypeStore bridge 补语义，top-level `val` 仍 lazy first-access，per-cone init routine 仍未执行真实 eager init。
  - 最终验收命令基线：`cargo fmt`；`cargo run -p scoop_tools -- dependency-gate`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`（完整 fixture suite，至少 30 分钟 timeout）；`cargo clippy --all-targets -- -D warnings`；`git diff --check`；并配套 residual 搜索 `rg -n "comptime|const_eval|devirtual|hir_compat_scaffold|llvm_residual_pass_view|materialized_pass_view|EffectLoweredStageOutput|EffectFactsStageOutput.*MirStageOutput|LirStageOutput.*MirStageOutput"`。
  - 未来 C backend 风险：如果 P6/P7 后 LLVM 仍需要读取 HIR/raw MIR/effect facts 或 backend-specific TypeStore bridge，未来 C backend 会复制同一耦合；因此 P7/P8 完成条件必须冻结为共享 `LIR + LIR facts + base context` 输入边界，而不是只让 LLVM 局部通过。

## [DONE] P6-T01：发布 global init 与 storage LIR facts contract

- 目标：
  - 在 `scoopc_lir_facts` 中发布 backend-neutral global init/storage contract；
  - 让 per-cone init routine、final entry init order、object once、top-level eager init 和 storage policy 有 LIR-owned 查询面。
- 必须修改的主要位置：
  - `crates/scoopc_lir_facts/`
  - `crates/scoopc/src/pipeline/lir_facts_builder.rs`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
  - `crates/scoopc/src/effect_lowered/`
  - 相关 dump/verifier/tests/fixtures
- 必须实现的内容：
  1. 新增 LIR facts 数据结构表达 global root identity、root kind、storage policy、initializer presence、dependency root identity、object once contract、top-level eager init contract、per-cone init routine contract 与 final entry init order contract。
  2. 从 P3/P5 显式输入构造这些 facts；禁止通过 P4 output 或 LLVM backend 回看 HIR/raw MIR 来补合同。
  3. `LirFacts` verifier 必须检查 root identity 唯一、dependency 指向已发布 root、object once 与 top-level eager init 分类互斥、`@Global` / `@ThreadLocal` storage policy 完整。
  4. stable dump 中展示 global init/storage facts，便于后续 P6/P7 fixture 回归。
  5. 如果发现 MIR facts 只发布 dependency count 不足以构造 LIR contract，必须在本任务内补齐正确 owner，而不是让 LLVM 使用旧 dependency 旁路。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_lir_facts`
  3. `cargo test -p scoopc --no-default-features lir_facts_builder`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  5. `git diff --check`
- 完成条件：
  - `LirFacts` 明确发布 global init/storage/final-entry 合同；
  - 后续 LLVM 可从 `LIR + lir_facts + base context` 查询 global init 所需的 backend-neutral 信息；
  - 没有新增 HIR/raw MIR/effect facts 的 backend 公共查询面。
- 依赖：TODO-6-INIT
- 完成记录：
  - 新增 MIR-owned initializer dependency facts，避免 LIR/LLVM 只能从 dependency count 或 raw MIR item 回推 dependency identity。
  - `scoopc_lir_facts` 新增 global init/storage fact group，发布 global root、root kind、storage policy、initializer presence、dependency identity、object once、top-level eager init、per-cone init routine 与 final entry init order contract。
  - LIR facts verifier 覆盖 global root count、dependency target、object once/top-level eager init 互斥、mutable/extern storage policy、cone routine 与 final entry routine 引用完整性。
  - `lir_facts_builder` 从 `MirFacts` 构造 global init/storage facts；stable dump 在存在 global roots 时展示 global init/storage section，后续 P6/P7 可从 `LIR + lir_facts + base context` 查询这些 backend-neutral 信息。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc --no-default-features lir_facts_builder`；`cargo test -p scoopc_mir_facts`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`git diff --check`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P6-T01R：Review global init/storage LIR facts contract

- 参考：P6-T01。
- 重点：
  - `LirFacts` 是否真正表达 P6 所需的 global init/storage 合同；
  - facts owner 是否正确，是否引入新的上游整包嵌套或 backend 回看；
  - verifier/dump/test 是否能暴露 dependency、root kind 和 storage policy drift。
- 验证：
  - 重新运行 P6-T01 的所有验证；
  - 额外搜索 `LirFacts` / `scoopc_lir_facts` 是否引用 stage crate、`MaterializedMir`、`EffectFactsStageOutput` 或 LLVM 类型。
- 完成条件：
  - review 结论明确写出：global init/storage LIR facts contract 可以支撑 P6-T02/P6-T03，或列出阻塞项并在本 review 内修复。
- 依赖：P6-T01
- 完成记录：
  - Review 结论：`LirFacts` 已发布 global root、root kind、storage policy、initializer presence、dependency identity、object once、top-level eager init、per-cone init routine 与 final entry init order 合同，可以作为 P6-T02/P6-T03 的输入边界继续推进。
  - 本 review 修复了 verifier 漏洞：现在会拒绝 top-level eager/object-once contract 与 root 的 initializer/storage drift、extern global contract 缺失或 initializer_absent drift、eager root 未进入或重复进入 cone init routine、routine/root cone 不一致、final entry routine 遗漏/重复，以及 eager dependency init order drift。
  - owner/依赖复审：`scoopc_lir_facts` 仍只依赖 `scoopc_ids`、`scoopc_project_model`、`scoopc_types`，额外搜索未发现 stage crate、`MaterializedMir`、`EffectFactsStageOutput` 或 LLVM 类型引用；global init/storage facts 从 MIR facts/LIR stage 显式输入构造，没有新增 backend 回看查询面。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc --no-default-features lir_facts_builder`；`cargo test -p scoopc_mir_facts`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；额外 residual 搜索 `MaterializedMir|EffectFactsStageOutput|inkwell|llvm|crate::mir|crate::effect|crate::hir|scoopc::|scoopc_mir|scoopc_effect|scoopc_hir` 于 `crates/scoopc_lir_facts` 无命中；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

## [DONE] P6-T02：实现 per-cone eager top-level init 与 final entry order

- 目标：
  - 让 top-level `val` 和 annotated top-level `var` 在用户 `main` body 前按 source-cone DAG / per-cone root contract eager 初始化；
  - 删除 top-level `val` lazy first-access 语义。
- 必须修改的主要位置：
  - `crates/scoopc/src/llvm/codegen/main/cone_init.rs`
  - `crates/scoopc/src/llvm/codegen/main/immut_value.rs`
  - `crates/scoopc/src/llvm/codegen/main/globals.rs`
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/effect_lowered/` 与 LIR global init lowering 相关位置
  - global init 相关 fixtures
- 必须实现的内容：
  1. per-cone init routine 必须根据 P6-T01 的 LIR facts 初始化本 cone roots，不递归替依赖 cone 干活。
  2. final system entry 必须在 runtime init 后、用户 `main` body 前按 linked source-cone DAG 顺序调用 cone init routines。
  3. top-level `val` 访问路径改为读取 eager 初始化后的 backing storage，不再触发 lazy init function / once guard。
  4. annotated top-level `var` initializer 进入 cone init routine；静态数据 initializer helper 只能作为 backend 物理化优化，不能改变 eager init 语义。
  5. 初始化 dependency contract 必须由 facts/IR 决定，不能依赖源码文件编译顺序或 LLVM 遍历偶然顺序。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features global_init`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `git diff --check`
- 完成条件：
  - top-level `val` / annotated top-level `var` 不再 lazy first-access；
  - 所有 linked cones 的 top-level eager init 在 `main` 前完成；
  - cone init routine 真实执行本 cone roots，且 entry order 有测试覆盖。
- 依赖：P6-T01R
- 完成记录：
  - LLVM cone init plan 改为消费 `LirFacts.global_init.final_entry_order` / `cone_init_routines`，不再从 HIR source map 或 side table 临时推导 routine/root 顺序；LIR facts builder 同步按 eager root dependency 为 final entry routines 做跨 cone 拓扑排序。
  - per-cone init routine 现在执行本 routine 发布的 top-level eager roots：top-level `val` 通过 eager init helper 在 main 前初始化，annotated top-level `var` 在 routine 内求值并写入 backing storage；top-level var storage 默认零初始化，initializer 不再要求 LLVM static const 路径。
  - top-level `val` 普通访问和 hidden bridge 不再 first-access 调用 init helper，只保留 initialized check 并读取 eager backing storage；递归初始化错误仍由 guard check 稳定失败。
  - MIR root facts 为 initializer/extern global root 使用 source path 对应的 source-cone stable key，避免 LIR global init facts 把所有 roots 归到单一 compilation cone。
  - 新增 LLVM IR 单测覆盖 cone routine 执行 eager roots 和普通 top-level `val` 访问不再 lazy-init；新增 `tests/fixtures/run-pass/global_init/eager_val_and_global_var_before_main.scoop`，并按 eager 语义更新既有 top-level `val` stdout golden。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc --no-default-features global_init`；`cargo test -p scoopc global_init`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoopc pipeline_user_visible_failure_policy_tracks_internal_bug_sentinels`；`git diff --check`。
  - 已运行完整 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：P6-T02 相关 fixtures 均通过，但全量 run-pass 仍有 7 个既有失败，分别来自 unresolved generic `scoop.core.println`、`scoop.runtime.test.*` import 或非本任务路径；未将这些历史问题作为 P6-T02 前置项。

## [DONE] P6-T02R：Review per-cone eager top-level init 与 final entry order

- 参考：P6-T02。
- 重点：
  - top-level eager init 是否覆盖 `val`、`@Global var`、entry-thread `@ThreadLocal var`；
  - final entry init order 是否只由 source-cone DAG 与 LIR facts 决定；
  - 是否删除或隔离了 top-level `val` lazy once path。
- 验证：
  - 重新运行 P6-T02 的所有验证；
  - 额外搜索 `immut_value` / global init code 中是否仍存在 top-level `val` first-access once 初始化路径。
- 完成条件：
  - review 结论明确写出 eager top-level init 与 final entry order 已闭合，或列出阻塞项并在本 review 内修复。
- 依赖：P6-T02
- 完成记录：
  - Review 结论：eager top-level init 与 final entry order 已闭合到 P6-T02R 要求；top-level `val` 普通访问不再 first-access lazy init，`@Global var` 与 entry-thread `@ThreadLocal var` initializer 均由 per-cone eager init routine 在用户 `main` 前执行。
  - 本 review 修复了两个阻塞缺口：MIR/LIR global init facts 现在携带 dependency-before-consumer source-cone topo order，LIR verifier 会拒绝 final entry routine source-cone 顺序倒置；HIR top-level initializer dependency 收集会递归扫描直接调用的顶层/成员函数体，避免经 helper 间接读取 top-level root 时漏排 eager init dependency。
  - 新增回归覆盖：`eager_threadlocal_var_before_main.scoop` 验证 entry-thread TLS initializer 在 `main` 前运行；`indirect_eager_dependency_via_function.scoop` 验证经函数间接读取的 eager dependency 会先初始化；`scoopc_lir_facts` verifier 单测覆盖 final-entry source order drift。
  - 残余搜索结论：`immut_value` / main global init 代码中，`ensure_top_level_immutable_value_init_function_defined` 只由 `cone_init.rs` 的 eager routine 调用；普通 top-level `val` 访问路径未再调用 lazy init helper。`scoop_once_begin/end` 仍存在于 top-level `val` init helper body 与 object once 路径，P6-T03 将继续分离 object once 与 storage policy。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc_mir_facts`；`cargo test -p scoopc --no-default-features global_init`；`cargo test -p scoopc --no-default-features hir_top_level_init_publishes_storage_and_extern_roots`；`cargo test -p scoopc --no-default-features lir_facts_builder`；`cargo test -p scoopc global_init`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 已重跑 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：新增/相关 global-init、top-level-val、top-level-var fixtures 均通过；全 run-pass 仍有 7 个既有非本任务失败（array literal inference、mutable array ref/composite、std process args、stdlib string、sysroot atomic 路径），未作为 P6-T02R 前置项。

## [DONE] P6-T03：分离 object once 与 `@Global` / `@ThreadLocal` storage policy

- 目标：
  - object singleton 保留独立 once 语义；
  - top-level eager init 不再共享 object once runtime path；
  - `@Global` / `@ThreadLocal` storage policy 在 HIR/MIR/LIR/codegen/runtime 语义上闭合。
- 必须修改的主要位置：
  - `crates/scoopc/src/llvm/codegen/object_init.rs`
  - `crates/scoopc/src/llvm/codegen/main/globals.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body/member.rs`
  - `runtime/c/scoop_once.c`
  - HIR/typecheck storage policy gate 与相关 fixtures
- 必须实现的内容：
  1. object once guard/runtime helper 只服务 object singleton，不再被 top-level `val` eager init 复用。
  2. object 初始化中的递归/半初始化 contract 与普通 self access 保持独立测试覆盖。
  3. `@Global var` 使用 global storage，并由 cone init routine 初始化。
  4. entry thread 的 `@ThreadLocal var` 使用 TLS storage，并由 cone init routine 初始化。
  5. 非 entry thread TLS 初始化策略必须显式实现；若本轮语义选择限制该能力，必须在 HIR/typecheck 屏障稳定拒绝并有 fixture，不能留给 LLVM/runtime 猜测。
  6. `@Extern` global / thread-local root 必须继续遵守 storage policy、unsafe 和 no-initializer 合同。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features storage_policy`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`
  5. `cargo test -p scoop_runtime --all-targets`
  6. `git diff --check`
- 完成条件：
  - object once 与 top-level eager init 不共享语义路径；
  - `@Global` / `@ThreadLocal` storage policy 从 frontend facts 到 codegen/runtime 有一致实现；
  - unsupported TLS 场景不再以后端 panic 或未定义行为暴露。
- 依赖：P6-T02R
- 完成记录：
  - top-level `val` eager init helper 不再调用 runtime `scoop_once_begin/end`，改用编译器私有 guard state machine；普通访问仍要求 guard 已 initialized，递归 eager init 继续稳定以退出码 1 失败。
  - object singleton init 继续独占 runtime once 路径；更新 runtime/codegen 注释并新增 LLVM 单测确认 top-level eager init 不含 `scoop_once`，object singleton init 仍保留 `scoop_once`。
  - `@Global var` 继续使用普通 LLVM global 并由 per-cone init routine 初始化；`@ThreadLocal var` 继续使用 LLVM TLS storage，entry thread 由 final entry cone init 初始化，worker thread 通过新生成的 `scoop_thread_init_current` hook 在 `scoop.thread` native trampoline 进入用户 closure 前初始化本线程 TLS roots。
  - HIR/typecheck storage policy gate 新增 `@ThreadLocal` 与 `@Global` 互斥诊断，避免 frontend facts/codegen 对冲突策略静默择一；`@Extern` global/TLS 无 initializer 与 storage policy 合同保持不变。
  - 新增 fixtures 覆盖 worker thread TLS 初始化、object once 普通访问、object recursive half-init contract，以及 storage policy 冲突诊断。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc --no-default-features storage_policy`；`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo test -p scoop_runtime --all-targets`；`cargo test -p scoopc llvm_top_level_eager_init_does_not_call_object_once_runtime`；`cargo test -p scoopc llvm_object_singleton_init_keeps_object_once_runtime`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 备注：首次 `cargo test -p scoop_runtime --all-targets` 使用 20 分钟工具 timeout 在 `stackmap_registry` 附近被中止；随后单独运行 `cargo test -p scoop_runtime --test stackmap_registry -- --nocapture` 和更长 timeout 的完整 runtime suite 均通过，未发现单个 runtime test 卡住。

## [DONE] P6-T03R：Review object once 与 storage policy 分离结果

- 参考：P6-T03。
- 重点：
  - object once 是否只服务 object；
  - top-level eager init 是否不再复用 once first-access；
  - `@Global` / `@ThreadLocal` / `@Extern` storage policy 是否在 HIR/MIR/LIR/codegen/runtime 一致。
- 验证：
  - 重新运行 P6-T03 的所有验证；
  - 额外搜索 `scoop_once` 调用点，确认 top-level eager init 没有继续走 object once helper。
- 完成条件：
  - review 结论明确写出 object once 与 storage policy 已闭合，或列出阻塞项并在本 review 内修复。
- 依赖：P6-T03
- 完成记录：
  - Review 结论：object once 与 storage policy 分离结果已闭合到 P6-T03R 要求；object singleton 继续独占 runtime `scoop_once_begin/end` 路径，top-level eager init 不再复用 once first-access，`@Global` / `@ThreadLocal` / `@Extern` storage policy 在 HIR/MIR/LIR/codegen/runtime 路径保持一致。
  - 复审结果：`declare_runtime_once_begin/end` 的实际 codegen 调用点只在 `object_init.rs`，top-level `val` eager init 通过 `immut_value.rs` 的 compiler-private guard state machine 和 `cone_init.rs` 的 eager routine 执行；普通 top-level `val` 访问只做 initialized check 后读取 backing storage。
  - Storage policy 复审：非 extern 顶层 `var` 仍由 typecheck 要求显式 `@Global` / `@ThreadLocal` 且拒绝二者冲突；HIR/MIR/LIR facts 保留 resolved storage policy；LLVM 对 `@Global` 使用普通 global，对 `@ThreadLocal` 使用 TLS storage；entry thread 经 final entry cone init 初始化 TLS roots，worker thread 经 `scoop_thread_init_current` hook 初始化当前线程 TLS roots；`@Extern` global/TLS 继续保持 no-initializer、unsafe 与 storage contract。
  - 额外 residual 搜索：`scoop_once_begin|scoop_once_end` 在 `crates/scoopc/src/llvm/codegen` 只剩 runtime symbol/declaration 注释、常量与 object guard 注释；`top_level_val_eager_init|top_level_val_init_guard|declare_runtime_once_begin` 搜索确认 top-level eager path 只调用 compiler-private init helper，runtime once declaration 只被 object init 使用。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc --no-default-features storage_policy`；`cargo test -p scoopc llvm_top_level_eager_init_does_not_call_object_once_runtime`；`cargo test -p scoopc llvm_object_singleton_init_keeps_object_once_runtime`；`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo test -p scoop_runtime --all-targets`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

## [DONE] P6-T04：P6 全包清场、文档同步与依赖审计

- 目标：
  - 对 P6 global init/storage/entry order 做最终清场；
  - 同步文档、README、fixtures 和 TODO 状态；
  - 为 P7 删除 LLVM HIR/raw MIR residual 提供稳定输入边界。
- 必须修改的主要位置：
  - `TODO.md`
  - `TODO-6.md`
  - `PIPELINE-CLEANUP.md`
  - `PIPELINE_REFACTOR.md`（仅在设计边界实际变化时）
  - `README.md`
  - global init/storage fixtures
- 必须实现的内容：
  1. 搜索确认 top-level `val` 不再 lazy first-access，per-cone init routine 不再为空壳。
  2. 搜索确认 object once 与 top-level eager init 语义路径已分离。
  3. 搜索确认 LIR facts 已发布 global init/storage/final-entry contract，LLVM 不再为 P6 语义创建新的 HIR fallback。
  4. 更新文档，明确 P7 仍需清理的 backend residual 只剩 HIR scaffold、raw MIR pass view、reachability、physical ABI/layout 与 TypeStore bridge。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_lir_facts`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `cargo clippy --all-targets -- -D warnings`
  6. `git diff --check`
- 完成条件：
  - P6 global init/storage/entry order 语义闭合；
  - `TODO.md` 与本文件状态同步；
  - P7 可以从明确的 `LIR + lir_facts + base context` global-init 输入开始清理 LLVM。
- 依赖：P6-T03R
- 完成记录：
  - 清场结论：P6 global init/storage/entry order 语义已闭合。top-level `val` 普通访问只做 initialized check 后读取 eager backing storage；`ensure_top_level_immutable_value_init_function_defined` 只由 `cone_init.rs` 的 per-cone eager routine 调用；per-cone routine 从 `LirFacts.global_init.final_entry_order` / `cone_init_routines` 构造并执行本 cone eager roots，不再是空壳。
  - object once 分离结论：`declare_runtime_once_begin/end` 的实际 codegen 调用点只剩 `object_init.rs`，top-level eager init 使用 compiler-private guard state machine，不再复用 runtime object once helper。
  - LIR facts 合同结论：`scoopc_lir_facts` 已发布 global root、storage policy、object once、top-level eager init、per-cone routine 与 final entry order contracts；`lir_facts_builder` 从 MIR facts 构造这些合同，verifier 覆盖 root/dependency/storage/routine/order drift，dependency gate 确认 fact crate 仍只依赖基础 crate。
  - 本任务修复了 P6 清场验证暴露的真实阻塞：`scoop.thread` native object 会引用 `scoop_thread_init_current`，而没有 TLS roots 的程序此前不会定义该 hook，导致线程相关 fixtures 链接失败。现在 LLVM 始终定义 `scoop_thread_init_current`，无 TLS roots 时生成 no-op body，有 TLS roots 时仍执行当前线程 TLS roots；`std_thread_basic`、delegated-property 并发 fixtures 与 GC cross-thread continuation fixture 因此恢复通过。
  - 文档同步：`README.md` 与 `PIPELINE-CLEANUP.md` 已更新为 P6 完成状态，明确 P7 剩余 residual 只应覆盖 HIR scaffold、raw MIR/pass-view body emission、reachability、physical ABI/layout、entry/global HIR side-table 过渡读取与多 `TypeStore` bridge。`PIPELINE_REFACTOR.md` 未修改，因为设计边界没有变化。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo test -p scoopc --no-default-features storage_policy`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 完整 run-pass：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已重跑，P6/global-init/thread-init 相关 fixtures 全部通过；全量仍有 7 个既有非 P6 失败（`array_lit_infer_unannotated_and_nested_basic.scoop`、`array_lit_infer_string_char_float_basic.scoop`、`lang_mutable_array_new_string_ref.scoop`、`lang_mutable_array_new_struct_composite.scoop`、`std_process_args_exit_basic.scoop`、`stdlib_string_basic.scoop`、`sysroot_atomic_basic.scoop`），与 P6-T02R / P5 记录的历史 baseline 一致，未作为 P6-T04 前置项。

## [DONE] P6-T04R：Review P6 全包完成度

- 参考：P6-T04。
- 重点：
  - P6 是否满足 `PLAN.md` 中 object once、top-level eager init、per-cone init routine、final entry order 与 storage policy 完成标准；
  - P7 residual 是否没有被错误描述为 P6 已完成范围。
- 验证：
  - 重新运行 P6-T04 的所有验证；
  - 额外搜索 global init/storage 相关 HIR scaffold 在 LLVM 中的剩余读取点，并确认它们已明确留给 P7。
- 完成条件：
  - review 结论明确写出 P6 完成、P7 可以开始，或列出阻塞项并在本 review 内修复。
- 依赖：P6-T04
- 完成记录：
  - Review 结论：P6 global init model 已满足 `PLAN.md` 中 object once、top-level eager init、per-cone init routine、final entry order 与 storage policy 完成标准；P7 可以从已明确的 `LIR + lir_facts + base context` global-init 输入边界开始清理 LLVM backend residual。
  - P6 完成度复审：`cone_init.rs` 按 `LirFacts.global_init.final_entry_order` / `cone_init_routines` 构造并执行 eager roots；top-level `val` 普通访问只做 initialized guard 检查并读取 backing storage；`@Global` / `@ThreadLocal` top-level `var` 由 cone/thread init routine 初始化；`scoop_thread_init_current` 在无 TLS roots 时也会生成 no-op hook，避免线程 runtime 链接漂移。
  - object once 复审：`scoop_once_begin/end` 的实际 codegen 调用点只在 `object_init.rs`，top-level eager init 继续使用 compiler-private guard state machine，没有复用 object once runtime helper。
  - P7 residual 复审：LLVM 中 `top_level_vars`、`top_level_immutable_values`、`object_inits`、`extern_globals` 等 HIR scaffold 读取仍存在于 `emit.rs`、`codegen/mod.rs`、`codegen/main/*`、`reachability.rs`、`mir_body/*` 与 `effect_lowered/*` 等路径；这些均已在 `P7-T01` / `P7-T02` / `P7-T03` / `P7-T04` 作为 backend cleanup residual 登记，未被误描述为 P6 已完成范围。
  - 额外 residual 搜索：`ensure_top_level_immutable_value_init_function_defined` 只由 `cone_init.rs` 的 eager root emission 调用；`scoop_once_begin|scoop_once_end|declare_runtime_once_begin|declare_runtime_once_end` 在 LLVM codegen 中除 runtime symbol/declaration/comment 外，只剩 `object_init.rs` 的 object singleton init 调用。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 完整 run-pass：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已重跑，global-init/thread-init 相关 fixtures 全部通过；全量仍有 7 个既有非 P6 失败（`array_lit_infer_string_char_float_basic.scoop`、`array_lit_infer_unannotated_and_nested_basic.scoop`、`lang_mutable_array_new_string_ref.scoop`、`lang_mutable_array_new_struct_composite.scoop`、`std_process_args_exit_basic.scoop`、`stdlib_string_basic.scoop`、`sysroot_atomic_basic.scoop`），与 P6-T04 baseline 一致，未作为 P6-T04R 前置项。

## [DONE] P7-T01：迁移 LLVM entry/global 查询到 LIR facts

- 目标：
  - 删除 LLVM entry/global 初始化路径对 HIR side tables 的语义依赖；
  - 让 entry selection、global root inventory、extern global contract 和 cone init 调用只读取 LIR/LIR facts/base context。
- 必须修改的主要位置：
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/main/cone_init.rs`
  - `crates/scoopc/src/llvm/codegen/main/globals.rs`
  - `crates/scoopc/src/llvm/codegen/main/immut_value.rs`
  - `crates/scoopc/src/llvm/codegen/object_init.rs`
  - `crates/scoopc_lir_facts/`
- 必须实现的内容：
  1. `select_entry_main` / entry argument classification 不再遍历 `LoweredHir.file.items`，改用 LIR callable facts 或明确 base context。
  2. extern global、top-level var、top-level val、object singleton 的 LLVM declaration/definition inventory 改读 LIR global init/storage facts。
  3. 移除或降级 `CompilationUnitCodegenCx` 中仅为 entry/global 查询保留的 HIR side table 字段。
  4. 如果发现 LIR facts 缺少 entry/global 所需信息，必须补 facts owner，而不是保留 HIR scaffold fallback。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features llvm_entry_global`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `git diff --check`
- 完成条件：
  - LLVM entry/global 语义查询不再依赖 HIR side tables；
  - global init/storage 的 backend 物理化只消费 LIR facts 与 base/type context；
  - 没有新增 backend-private HIR fallback。
- 依赖：P6-T04R
- 完成记录：
  - `scoopc_lir_facts` 补齐 LLVM entry/global 查询需要的 backend-neutral owner：`LirCallableFacts` 现在发布 source-level callable kind、source 参数类型和返回类型，`LirExternGlobalFacts` 发布 extern linkage；verifier 会拒绝 callable source signature 与 ABI contract 漂移。
  - LLVM entry selection / `main(args: Array<String>)` 参数分类改为读取 LIR callable facts + TypeStore，不再遍历 `LoweredHir.file.items` 来决定入口语义；HIR `FunDecl` 只作为当前 P7-T03 前仍存在的 body emission scaffold 按已选 FQN 取回。
  - extern global、top-level `var`、top-level `val`、object singleton 的 LLVM declaration/definition inventory 查询改为读取 `LirFacts.global_init`；`CompilationUnitCodegenCx` 已移除 HIR `extern_globals` side table 输入，top-level/object HIR side tables 仅保留 initializer/body emission scaffold residual。
  - 新增 LLVM entry/global 回归测试覆盖 LIR callable facts 驱动的 argv entry classification，以及 extern global symbol declaration/load-store 由 LIR global facts physicalize。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc --no-default-features llvm_entry_global`（无匹配 LLVM 测试但编译通过）；`cargo test -p scoopc llvm_entry_global`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 完整 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已重跑：entry/global、extern global、global-init 相关 fixtures 均通过；全量仍有 7 个既有非 P7-T01 失败（`array_lit_infer_unannotated_and_nested_basic.scoop`、`array_lit_infer_string_char_float_basic.scoop`、`lang_mutable_array_new_string_ref.scoop`、`lang_mutable_array_new_struct_composite.scoop`、`std_process_args_exit_basic.scoop`、`stdlib_string_basic.scoop`、`sysroot_atomic_basic.scoop`），与 P6-T04R baseline 一致，未作为本任务前置项。

## [DONE] P7-T01R：Review LLVM entry/global LIR facts 迁移结果

- 参考：P7-T01。
- 重点：
  - LLVM entry/global 查询是否已切到 LIR facts；
  - P6 global init contract 是否未被 LLVM 重新推导；
  - HIR side table residual 是否只剩后续 body/reachability/physical layout 范围。
- 验证：
  - 重新运行 P7-T01 的所有验证；
  - 额外搜索 `emit.rs` / `codegen/main` 中对 `top_level_vars`、`top_level_immutable_values`、`object_inits`、`extern_globals` 的 HIR scaffold 读取。
- 完成条件：
  - review 结论明确写出 entry/global backend 输入边界已迁移，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T01
- 完成记录：
  - Review 结论：LLVM entry/global backend 输入边界已迁移到 `LIR + lir_facts + base/type context`；entry main selection / `main(args: Array<String>)` 参数分类从 `LirCallableFacts` 与 `TypeStore` 查询，不再遍历 `LoweredHir.file.items` 做入口语义判定。
  - global inventory 复审：per-cone init plan 由 `LirFacts.global_init.final_entry_order` / `cone_init_routines` / `roots` 驱动；top-level `var`、top-level `val`、object singleton 与 `@Extern` global 的 LLVM storage/key/declaration 入口均先校验或读取 `LirGlobalRootFacts`，没有新增 backend-private HIR fallback。
  - residual 搜索结论：`emit.rs` 中的 `top_level_vars` / `top_level_immutable_values` / `object_inits` 传入仍用于 body/initializer emission scaffold，`extern_globals` 只剩 `reachability` 输入；`codegen/main` 中未发现 `extern_globals` 读取，剩余 `top_level_vars` / `top_level_immutable_values` 读取均在 LIR facts 选中 root 后取 initializer/body scaffold，归属 P7-T02/P7-T03 后续清理范围。
  - LIR facts 复审：`scoopc_lir_facts` verifier 覆盖 callable source signature 与 ABI contract 漂移、global root/storage/extern contract drift、eager root/routine/final-entry order drift；生产 `lir_facts_builder` 构造后执行 `facts.verify()`，未发现 P7-T01R 阻塞项。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc --no-default-features llvm_entry_global`（无匹配测试但编译通过）；`cargo test -p scoopc llvm_entry_global`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 完整 run-pass：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已重跑，entry/global、extern global、global-init 相关 fixtures 通过；全量仍有 7 个既有非 P7-T01R 失败（`array_lit_infer_unannotated_and_nested_basic.scoop`、`array_lit_infer_string_char_float_basic.scoop`、`lang_mutable_array_new_string_ref.scoop`、`lang_mutable_array_new_struct_composite.scoop`、`std_process_args_exit_basic.scoop`、`stdlib_string_basic.scoop`、`sysroot_atomic_basic.scoop`），与 P7-T01 / P6 baseline 一致，未作为本 review 前置项。

## [DONE] P7-T02：删除 backend reachability HIR/MIR 回看与 codegen 去虚化 residual

- 目标：
  - 让 LLVM reachability 只基于 LIR/LIR facts 的 callable、dispatch、dynamic invoke 与 global init root；
  - 删除 reachability 层临时去虚化推断和 HIR/raw MIR 扫描。
- 必须修改的主要位置：
  - `crates/scoopc/src/llvm/reachability.rs`
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/lookup.rs`
  - `crates/scoopc_lir_facts/`
  - reachability 相关 tests/fixtures
- 必须实现的内容：
  1. reachability seed 来自 final entry、global init routines、published LIR callables 和 runtime-required callables。
  2. ordinary/dynamic/dispatch edges 来自 LIR facts，不从 HIR body、raw MIR body或 pass-view candidate body 推断。
  3. 删除 backend reachability 中的 devirtualization fallback；去虚化 owner 仍只能是 MIR 或 LIR 窄 state-machine opt。
  4. reachability dump/test 能覆盖 object/global init root、dynamic invoke、dispatch slot 与 continuation/resume callable。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features llvm::reachability`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `git diff --check`
- 完成条件：
  - `llvm/reachability.rs` 不再读取 HIR body、raw MIR body或 `MaterializedMirPassView`；
  - codegen 层不再做普通 dispatch devirtualization；
  - LIR/LIR facts reachability 足以覆盖 LLVM emission。
- 依赖：P7-T01R
- 完成记录：
  - `llvm/reachability.rs` 已替换为只消费 `LirFacts` 的 collector：入口 root、global init roots、已发布 LIR callables、runtime-required callables 作为 seeds；ordinary call、dynamic invoke、dispatch candidate edges 均从 `LirCallableFacts` / `LirDynamicInvokeContract` / `LirDispatchContract` 读取，不再导入或扫描 HIR body、raw MIR body、`MaterializedMirPassView`。
  - reachability 对 `KnownInstance` target 保持 required 语义，缺失时继续暴露为 backend handoff 错误；对 conservative `CandidateSet` / dispatch candidate 只 enqueue 已发布到 `LirFacts.callables` 的 callable，避免把未发布的宽候选（例如无关 `ToString` implementer）重新作为 backend HIR fallback 目标。
  - `emit.rs` 的 reachable set 改为从 LIR facts reachable FQN 映射到当前 P7-T03 前仍存在的 legacy body/declaration scaffold；删除了原先基于 HIR `fun_index` 的 monomorphized helper 追加逻辑。
  - 删除 `llvm/codegen/call/lowering.rs` 中接口调用的 codegen 层 dispatch 去虚化 fallback；普通 dispatch target refinement 只保留在 MIR/LIR-owned pass/facts 路径。
  - 新增 `llvm::reachability` 单测覆盖 LIR callable edges、global init root seed、dynamic invoke、dispatch target、unpublished conservative candidate 忽略，以及 published continuation/resume callable seed。
  - residual 搜索 `hir::|mir::|MaterializedMirPassView|devirtual` 于 `crates/scoopc/src/llvm/reachability.rs` 无命中；LLVM codegen 下 `devirtual|try_devirtualize|collect_known_receiver_subclasses|dispatch_devirtualization` 无命中。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc --no-default-features llvm::reachability`（LLVM feature-gated，无匹配测试但命令通过）；`cargo test -p scoopc llvm::reachability`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 完整 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已用 30 分钟 timeout 重跑：414/421 通过；剩余 7 个失败为既有非 P7-T02 baseline（`array_lit_infer_string_char_float_basic.scoop`、`array_lit_infer_unannotated_and_nested_basic.scoop`、`lang_mutable_array_new_string_ref.scoop`、`lang_mutable_array_new_struct_composite.scoop`、`std_process_args_exit_basic.scoop`、`stdlib_string_basic.scoop`、`sysroot_atomic_basic.scoop`）。

## [TODO] P7-T02-a：修复 run-pass fixture baseline 失败

- 目标：
  - 修复 P6/P7 期间反复登记的 7 个既有 run-pass fixture baseline 失败；
  - 让 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 不再保留这些非 P6/P7 baseline 失败。
- 已知失败 fixture：
  1. `tests/fixtures/run-pass/array_lit_infer_unannotated_and_nested_basic.scoop`
  2. `tests/fixtures/run-pass/array_lit_infer_string_char_float_basic.scoop`
  3. `tests/fixtures/run-pass/lang_mutable_array_new_string_ref.scoop`
  4. `tests/fixtures/run-pass/lang_mutable_array_new_struct_composite.scoop`
  5. `tests/fixtures/run-pass/std_process_args_exit_basic.scoop`
  6. `tests/fixtures/run-pass/stdlib_string_basic.scoop`
  7. `tests/fixtures/run-pass/sysroot_atomic_basic.scoop`
- 当前复现到的主要失败原因：
  1. 多数 fixture 在 LLVM 前端准备阶段失败：materialized MIR 中仍有 unresolved generic direct call target `scoop.core.println`。
  2. `lang_mutable_array_new_string_ref.scoop` 直接 `scoop run` 时会先暴露 `scoop.runtime.test.*` import 未解析；通过 fixture harness / `SYSROOT-DEPS` 注入后同样会落到 `scoop.core.println` 泛型 materialization 失败。
- 必须修改的主要位置：
  - `crates/scoopc/src/mir/materialize/`
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 或 LLVM frontend prepare 相关入口
  - `sysroot/lib/scoop.core/src/print.scoop` 及 generic `println<T>` materialization 所需 facts/实例化通道
  - `crates/scoop/src/fixtures/` 与 sysroot dependency 注入路径（仅当 `SYSROOT-DEPS` 未被正确传递时）
  - 上述 7 个 run-pass fixtures / golden（仅在语义输出确有变化时）
- 必须实现的内容：
  1. 定位为什么这些 call site 在 materialized MIR 中仍指向泛型 `scoop.core.println`，并修复实例化/替换/发布链路；不得在 LLVM backend 恢复 HIR fallback 或把 unresolved generic call 当作 codegen 特例兜底。
  2. 确认数组字面量、`MutableArray<T>`、`String` 方法、`main(args)` 与 Atomic sysroot API 相关路径都能获得 concrete callable target。
  3. 复查 `SYSROOT-DEPS: scoop.runtime.test` 是否在 run-pass harness 进入 `scoop run` 子进程时正确传递；若传递已正确，只需在完成记录说明直接 `scoop run` 与 fixture harness 行为差异。
  4. 如果修复需要新增 LIR/LIR facts 或 MIR materialization contract，必须放在正确 owner，不允许把 P7 backend cleanup 变成新的上游回看入口。
- 验证：
  1. `cargo fmt`
  2. 逐个运行上述 7 个 fixtures：`cargo run -p scoop -- test --fixtures <fixture>`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  4. `cargo clippy --all-targets -- -D warnings`
  5. `git diff --check`
- 完成条件：
  - 上述 7 个 fixtures 全部通过；
  - 全量 `tests/fixtures/run-pass` 不再保留这些 baseline 失败；
  - 修复没有新增 LLVM HIR/raw MIR fallback、codegen 去虚化 residual 或 stage output wrapper 回退。
- 依赖：P7-T02
- 完成记录：
  - 待填写。

## [TODO] P7-T02R：Review backend reachability cleanup

- 参考：P7-T02。
- 重点：
  - reachability 是否只读 LIR/LIR facts/base context；
  - 是否删除 codegen/reachability devirtualization residual；
  - 是否没有把 MIR pass-view 换名藏入 reachability helper。
- 验证：
  - 重新运行 P7-T02 的所有验证；
  - 额外搜索 `llvm/reachability.rs` 中的 `hir::`、`mir::`、`MaterializedMirPassView`、`devirtual`。
- 完成条件：
  - review 结论明确写出 backend reachability cleanup 成立，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T02-a
- 完成记录：
  - 待填写。

## [TODO] P7-T03：迁移 LLVM body emission 离开 raw MIR / HIR fallback

- 目标：
  - 让 LLVM body emission 使用 LIR body/source-slice/callable contracts；
  - 删除 `mir_body` raw MIR lowering 与 `main` HIR fallback lowering 的生产依赖。
- 必须修改的主要位置：
  - `crates/scoopc/src/llvm/codegen/mir_body/`
  - `crates/scoopc/src/llvm/codegen/main/function.rs`
  - `crates/scoopc/src/llvm/codegen/main/expr.rs`
  - `crates/scoopc/src/llvm/codegen/main/stmt.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/body/`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc_lir_facts/`
- 必须实现的内容：
  1. effect-lowered body emission 不再从 pass view 拉 MIR function/body/local/type 信息；所需 source-slice/local/frame/signature 信息必须来自 LIR/LIR facts/base type context。
  2. 删除或测试专用化 `mir_body` production route；不允许 raw MIR body 作为 LLVM fallback。
  3. 删除 HIR expr/stmt/function/class ctor production fallback；静态源码错误不得在 backend 才暴露。
  4. 若 LIR 缺少 body emission 所需 contract，补到 P5/P6 owner 的 LIR facts，而不是保留 backend HIR/MIR 回看。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `git diff --check`
- 完成条件：
  - LLVM production body emission 不再读取 HIR body 或 raw MIR body；
  - `llvm_residual_pass_view()` 不再用于 body emission；
  - LIR/LIR facts 是 body emission 的唯一 authoritative IR/query 输入。
- 依赖：P7-T02R
- 完成记录：
  - 待填写。

## [TODO] P7-T03R：Review LLVM body emission 迁移结果

- 参考：P7-T03。
- 重点：
  - production body emission 是否已离开 raw MIR/HIR；
  - remaining MIR/HIR imports 是否仅为测试 fixture 或已删除；
  - LIR facts 是否没有变成 MIR/HIR 结构的简单搬运。
- 验证：
  - 重新运行 P7-T03 的所有验证；
  - 额外搜索 `llvm/codegen` 中的 `hir::Expr`、`hir::Stmt`、`mir::Body`、`MaterializedMirPassView`、`llvm_residual_pass_view`。
- 完成条件：
  - review 结论明确写出 LLVM body emission 输入边界已收口，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T03
- 完成记录：
  - 待填写。

## [TODO] P7-T04：收窄 LLVM stage handoff、physical ABI layout 与 TypeStore bridge

- 目标：
  - 删除 `LlvmCodegenStageOutput` / `StageEmitInput` 对 P5 wrapper、HIR scaffold 和 residual pass view 的长期携带；
  - 保留 LLVM backend-private physical ABI/layout，但其 logical 输入必须是 `LIR + LIR facts + base/type context`。
- 必须修改的主要位置：
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/ty.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
- 必须实现的内容：
  1. 引入或收口 LLVM backend input，使其只携带 LIR program、LIR facts、必要 base context / type context 和 backend options。
  2. `LlvmCodegenStageOutput` 不再保存 `LoweredHir`、`HirFacts` 或 `EffectLoweredStageOutput` wrapper。
  3. `StageEmitInput` 不再接收 primary/ABI visibility P5 wrapper；ABI visibility 需要的内容必须是明确的 LIR/LIR facts 输入。
  4. 删除 `LirStageOutput` 中仅为 LLVM residual 保留的 pass-view/effect context字段或 accessor。
  5. physical ABI/layout 中仍需的 class/vtable/itable/enum/type identity 必须来自 LIR facts 或 base type context；多 `TypeStore` 桥接必须有单一 owner 和 verifier。
  6. 在收口多 `TypeStore` 桥接的同时，评估 `TypeId` / type 身份的 cross-process stable wire format：未来 per-cone build artifact 落地（把每个 cone 的 `LIR + LIR facts` 持久化到 `build/<profile>/cones/<cone>/`，让下游 cone 不再重扫上游源码）需要 `TypeId` 或等价 type 身份在序列化/反序列化之间保持稳定。本任务作为 TypeStore 唯一 owner 的收口节点是最自然的处置点：要么把 fact/LIR 中的 `TypeId` 替换为 `scoopc_ids::CanonicalTextKey` 等 stable key，要么为 `TypeStore` 设计 portable serialization + import 重映射；若选择推迟，必须显式记录推迟原因和未来 owner，避免 P8 验收后再次散落到 backend 局部修补。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features llvm_codegen_stage`
  3. `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`
  4. `cargo run -p scoop_tools -- dependency-gate`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  6. `git diff --check`
- 完成条件：
  - LLVM stage wrapper 不再嵌套或传播上游整包 stage output；
  - `llvm_residual_pass_view()` 等 P7 residual accessor 删除；
  - physical ABI/layout 只把 LIR facts 映射成 LLVM-private layout，不回读 HIR/raw MIR/effect facts；
  - TypeStore 桥接收口结论中已经显式表态 cross-process stable wire format 的处置（落实或显式推迟 + owner）。
- 依赖：P7-T03R
- 完成记录：
  - 待填写。

## [TODO] P7-T04R：Review LLVM stage handoff 与 physical ABI cleanup

- 参考：P7-T04。
- 重点：
  - `LlvmCodegenStageOutput` / `StageEmitInput` 是否不再传播 P5 wrapper 或 HIR scaffold；
  - `LirStageOutput` 是否不再保留 LLVM residual pass-view context；
  - physical ABI/layout 是否只做 backend-private 物理化；
  - TypeStore 桥接收口结论是否已显式表态 cross-process stable wire format 的处置（落实或显式推迟 + 未来 owner），未来 per-cone build artifact 落地不会因为 `TypeId` 不可序列化而被迫重新触动 backend。
- 验证：
  - 重新运行 P7-T04 的所有验证；
  - 额外搜索 `hir_compat_scaffold`、`llvm_residual_pass_view`、`EffectLoweredStageOutput`、`MaterializedMirPassView`、`EffectFactsStageOutput` 在 LLVM/pipeline handoff 中的生产命中。
- 完成条件：
  - review 结论明确写出 LLVM backend 输入边界已收口，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T04
- 完成记录：
  - 待填写。

## [TODO] P7-T05：P7 全包清场、文档同步与依赖审计

- 目标：
  - 对 LLVM backend cleanup 做最终清场；
  - 确认 codegen backend 只依赖 `LIR + LIR facts + base context`；
  - 同步文档、TODO 和 dependency gate。
- 必须修改的主要位置：
  - `TODO.md`
  - `TODO-6.md`
  - `PIPELINE-CLEANUP.md`
  - `PIPELINE_REFACTOR.md`（仅在设计边界实际变化时）
  - `README.md`
  - `tools/scoop_tools/src/dependency_gate.rs`（如需新增 backend 边界门禁）
- 必须实现的内容：
  1. 搜索确认 LLVM backend 无 HIR body/raw MIR body/effect facts/stage output wrapper 输入。
  2. 搜索确认 codegen/reachability 无 ordinary dispatch devirtualization residual。
  3. dependency gate 覆盖或记录 LLVM backend 输入边界，避免后续回退。
  4. 文档中明确 LLVM 与未来 C backend 共享同一套 backend-neutral 输入。
- 验证：
  1. `cargo fmt`
  2. `cargo run -p scoop_tools -- dependency-gate`
  3. `cargo test -p scoopc_lir_facts`
  4. `cargo test -p scoopc --no-default-features llvm_codegen_stage`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  6. `cargo clippy --all-targets -- -D warnings`
  7. `git diff --check`
- 完成条件：
  - P7 backend cleanup 完成；
  - `TODO.md` 与本文件状态同步；
  - P8 可以只做最终 residual 搜索、全仓验证和接口冻结。
- 依赖：P7-T04R
- 完成记录：
  - 待填写。

## [TODO] P7-T05R：Review P7 全包完成度

- 参考：P7-T05。
- 重点：
  - LLVM backend 是否只消费 `LIR + LIR facts + base context`；
  - 是否没有 HIR/raw MIR/effect facts/stage output wrapper residual；
  - P8 是否只剩最终验证和文档冻结。
- 验证：
  - 重新运行 P7-T05 的所有验证；
  - 额外搜索 `crates/scoopc/src/llvm` 与 `crates/scoopc/src/pipeline` 中的上游 stage output / HIR / MIR residual。
- 完成条件：
  - review 结论明确写出 P7 完成、P8 可以开始，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T05
- 完成记录：
  - 待填写。

## [TODO] P8-T01：最终 residual 搜索、文档冻结与未来 C backend 输入边界

- 目标：
  - 全仓确认 pipeline refactor 不再保留旧 surface、旧 owner 或 stage output 嵌套；
  - 冻结 LLVM 与未来 C backend 的共享输入边界。
- 必须修改的主要位置：
  - `TODO.md`
  - `TODO-6.md`
  - `PLAN.md`（仅当阶段计划实际变化）
  - `PIPELINE_REFACTOR.md`
  - `PIPELINE-CLEANUP.md`
  - `README.md`
  - crate overview / dependency gate 文档
- 必须实现的内容：
  1. 搜索确认无旧 comptime/const surface 或专门兼容逻辑。
  2. 搜索确认无 HIR 层去虚化、无 codegen/reachability 普通去虚化。
  3. 搜索确认无 `StageOutput` 嵌套上游整包或下游把两个 fact table 当替代输入。
  4. 文档冻结未来 C backend 的输入：`LIR + LIR facts + base context`，不允许复制 LLVM 旧 HIR/raw MIR coupling。
  5. 若 dependency gate 还不能覆盖最终约束，补齐 gate 或在 TODO 中留下明确阻塞项，不能只靠人工记忆。
- 验证：
  1. `cargo fmt`
  2. `cargo run -p scoop_tools -- dependency-gate`
  3. `cargo run -p scoop_tools -- spec-fixtures check`
  4. `cargo test --all --all-targets`
  5. `git diff --check`
- 完成条件：
  - 文档、TODO、README 与实现边界一致；
  - 最终 residual 搜索无违反 P0-P8 硬约束的生产命中；
  - 未来 C backend 输入边界明确且可被 dependency gate 或文档验收。
- 依赖：P7-T05R
- 完成记录：
  - 待填写。

## [TODO] P8-T01R：Review final residual 搜索与文档冻结

- 参考：P8-T01。
- 重点：
  - residual 搜索是否覆盖 P0-P8 所有硬约束；
  - 文档是否没有把 workaround 或 historical residual 描述成长期合法边界；
  - 未来 C backend 输入边界是否足够具体。
- 验证：
  - 重新运行 P8-T01 的所有验证；
  - 抽查 residual 搜索命令和 dependency gate 规则是否能防止明显回退。
- 完成条件：
  - review 结论明确写出 residual/documentation freeze 成立，或列出阻塞项并在本 review 内修复。
- 依赖：P8-T01
- 完成记录：
  - 待填写。

## [TODO] P8-T02：最终全仓验证与 release readiness 清场

- 目标：
  - 运行最终验证矩阵；
  - 确认所有 P0-P8 任务完成记录、TODO 索引、文档和实现状态一致；
  - 为首次 release tag 前的最终自动化验收留出干净状态。
- 必须修改的主要位置：
  - `TODO.md`
  - `TODO-6.md`
  - 任何最终验证发现不一致的源码、fixture 或文档
- 必须实现的内容：
  1. 全量运行最终验证命令，任何失败必须修复后重跑对应范围。
  2. 确认所有任务标题状态与完成记录一致；完成记录有内容但标题未 `[DONE]` 的任务必须修正。
  3. 确认没有未提交的临时 fixture、golden、debug dump 或 plan workaround。
  4. 如果全量 fixture 中任何单个测试卡住超过 1 分钟，必须定位并修复，不得仅跳过。
- 验证：
  1. `cargo fmt`
  2. `cargo run -p scoop_tools -- dependency-gate`
  3. `cargo run -p scoop_tools -- spec-fixtures check`
  4. `cargo test --all --all-targets`
  5. `cargo run -p scoop -- test`（完整 fixture suite，至少 30 分钟 timeout）
  6. `cargo clippy --all-targets -- -D warnings`
  7. `git diff --check`
- 完成条件：
  - 最终验证矩阵通过；
  - `TODO.md` 与所有 `TODO-[1-6].md` 状态同步；
  - 代码、fixtures、文档和 dependency gate 不再暴露 P0-P8 residual。
- 依赖：P8-T01R
- 完成记录：
  - 待填写。

## [TODO] P8-T02R：Review final verification 与 release readiness

- 参考：P8-T02。
- 重点：
  - 最终验证是否完整通过；
  - P0-P8 是否没有遗漏的未完成任务或未跟踪 blocker；
  - release tag 前是否还有必须修复的工作区、文档或测试风险。
- 验证：
  - 重新运行 P8-T02 的所有验证，或在验证成本过高时至少重跑失败修复相关范围并明确引用最近一次完整通过记录；
  - 复查 `git status`、TODO 状态、residual 搜索结果。
- 完成条件：
  - review 结论明确写出项目实现完成并可进入最终 release tag 流程，或列出阻塞项并在本 review 内修复。
- 依赖：P8-T02
- 完成记录：
  - 待填写。
